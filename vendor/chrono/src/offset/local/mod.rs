// This is a part of Chrono.
// See README.md and LICENSE.txt for details.

//! The local (system) time zone, delegated to `jiff` `0.2` (`#![forbid(unsafe_code)]`).

#[cfg(any(
    feature = "rkyv",
    feature = "rkyv-16",
    feature = "rkyv-32",
    feature = "rkyv-64"
))]
use rkyv::{Archive, Deserialize, Serialize};

use super::fixed::FixedOffset;
use super::{MappedLocalTime, TimeZone};
use crate::naive::{NaiveDate, NaiveDateTime, NaiveTime};
#[allow(deprecated)]
use crate::Date;
use crate::{DateTime, Utc};

#[cfg(all(
    feature = "clock",
    not(all(
        target_arch = "wasm32",
        not(any(target_os = "emscripten", target_os = "wasi"))
    ))
))]
mod inner {
    use crate::{Datelike, FixedOffset, MappedLocalTime, NaiveDateTime, Timelike};

    pub(super) fn offset_from_utc_datetime_in_tz(
        tz: &jiff::tz::TimeZone,
        utc: &NaiveDateTime,
    ) -> MappedLocalTime<FixedOffset> {
        let utc_dt = utc.and_utc();
        let secs = utc_dt.timestamp();
        let nanos = (utc_dt.timestamp_subsec_nanos() % 1_000_000_000) as i32;

        let ts = if secs < jiff::Timestamp::MIN.as_second() {
            jiff::Timestamp::MIN
        } else if secs > jiff::Timestamp::MAX.as_second() {
            jiff::Timestamp::MAX
        } else {
            jiff::Timestamp::new(secs, nanos).unwrap_or(if secs < 0 {
                jiff::Timestamp::MIN
            } else {
                jiff::Timestamp::MAX
            })
        };

        let offset_secs = tz.to_offset(ts).seconds();
        MappedLocalTime::Single(
            FixedOffset::east_opt(offset_secs).unwrap_or_else(|| FixedOffset::east_opt(0).unwrap()),
        )
    }

    pub(super) fn offset_from_local_datetime_in_tz(
        tz: &jiff::tz::TimeZone,
        local: &NaiveDateTime,
    ) -> MappedLocalTime<FixedOffset> {
        let Ok(year) = i16::try_from(local.year()) else {
            return offset_from_utc_datetime_in_tz(tz, local);
        };
        if !(-9999..=9999).contains(&year) {
            return offset_from_utc_datetime_in_tz(tz, local);
        }

        let month = local.month() as i8;
        let day = local.day() as i8;
        let hour = local.hour() as i8;
        let minute = local.minute() as i8;
        let second = local.second().min(59) as i8;
        let subsec_nanos = (local.nanosecond() % 1_000_000_000) as i32;

        let Ok(dt) =
            jiff::civil::DateTime::new(year, month, day, hour, minute, second, subsec_nanos)
        else {
            return offset_from_utc_datetime_in_tz(tz, local);
        };

        match tz.to_ambiguous_zoned(dt).offset() {
            jiff::tz::AmbiguousOffset::Unambiguous { offset } => MappedLocalTime::Single(
                FixedOffset::east_opt(offset.seconds())
                    .unwrap_or_else(|| FixedOffset::east_opt(0).unwrap()),
            ),
            jiff::tz::AmbiguousOffset::Fold { before, after } => MappedLocalTime::Ambiguous(
                FixedOffset::east_opt(before.seconds())
                    .unwrap_or_else(|| FixedOffset::east_opt(0).unwrap()),
                FixedOffset::east_opt(after.seconds())
                    .unwrap_or_else(|| FixedOffset::east_opt(0).unwrap()),
            ),
            jiff::tz::AmbiguousOffset::Gap { .. } => MappedLocalTime::None,
        }
    }

    pub(super) fn offset_from_utc_datetime(utc: &NaiveDateTime) -> MappedLocalTime<FixedOffset> {
        offset_from_utc_datetime_in_tz(&jiff::tz::TimeZone::system(), utc)
    }

    pub(super) fn offset_from_local_datetime(
        local: &NaiveDateTime,
    ) -> MappedLocalTime<FixedOffset> {
        offset_from_local_datetime_in_tz(&jiff::tz::TimeZone::system(), local)
    }
}

#[cfg(all(
    target_arch = "wasm32",
    not(any(target_os = "emscripten", target_os = "wasi")),
    feature = "wasmbind"
))]
mod inner {
    use crate::{Datelike, FixedOffset, MappedLocalTime, NaiveDateTime, Timelike};

    pub(super) fn offset_from_utc_datetime(utc: &NaiveDateTime) -> MappedLocalTime<FixedOffset> {
        let offset = js_sys::Date::from(utc.and_utc()).get_timezone_offset();
        MappedLocalTime::Single(FixedOffset::west_opt((offset as i32) * 60).unwrap())
    }

    pub(super) fn offset_from_local_datetime(
        local: &NaiveDateTime,
    ) -> MappedLocalTime<FixedOffset> {
        let mut year = local.year();
        if year < 100 {
            // The API in `js_sys` does not let us create a `Date` with negative years.
            // And values for years from `0` to `99` map to the years `1900` to `1999`.
            // Shift the value by a multiple of 400 years until it is `>= 100`.
            let shift_cycles = (year - 100).div_euclid(400);
            year -= shift_cycles * 400;
        }
        let js_date = js_sys::Date::new_with_year_month_day_hr_min_sec(
            year as u32,
            local.month0() as i32,
            local.day() as i32,
            local.hour() as i32,
            local.minute() as i32,
            local.second() as i32,
            // ignore milliseconds, our representation of leap seconds may be problematic
        );
        let offset = js_date.get_timezone_offset();
        // We always get a result, even if this time does not exist or is ambiguous.
        MappedLocalTime::Single(FixedOffset::west_opt((offset as i32) * 60).unwrap())
    }
}

#[cfg(any(
    all(
        target_arch = "wasm32",
        not(any(target_os = "emscripten", target_os = "wasi")),
        not(feature = "wasmbind")
    ),
    all(
        not(all(
            target_arch = "wasm32",
            not(any(target_os = "emscripten", target_os = "wasi"))
        )),
        not(feature = "clock")
    )
))]
mod inner {
    use crate::{FixedOffset, MappedLocalTime, NaiveDateTime};

    pub(super) fn offset_from_utc_datetime(
        _utc_time: &NaiveDateTime,
    ) -> MappedLocalTime<FixedOffset> {
        MappedLocalTime::Single(FixedOffset::east_opt(0).unwrap())
    }

    pub(super) fn offset_from_local_datetime(
        _local_time: &NaiveDateTime,
    ) -> MappedLocalTime<FixedOffset> {
        MappedLocalTime::Single(FixedOffset::east_opt(0).unwrap())
    }
}

/// The local timescale.
///
/// Using the [`TimeZone`](./trait.TimeZone.html) methods
/// on the Local struct is the preferred way to construct `DateTime<Local>`
/// instances.
///
/// # Example
///
/// ```
/// use chrono::{DateTime, Local, TimeZone};
///
/// let dt1: DateTime<Local> = Local::now();
/// let dt2: DateTime<Local> = Local.timestamp_opt(0, 0).unwrap();
/// assert!(dt1 >= dt2);
/// ```
#[derive(Copy, Clone, Debug)]
#[cfg_attr(
    any(
        feature = "rkyv",
        feature = "rkyv-16",
        feature = "rkyv-32",
        feature = "rkyv-64"
    ),
    derive(Archive, Deserialize, Serialize),
    archive(compare(PartialEq)),
    archive_attr(derive(Clone, Copy, Debug))
)]
#[cfg_attr(feature = "rkyv-validation", archive(check_bytes))]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Local;

impl Local {
    /// Returns a `Date` which corresponds to the current date.
    #[deprecated(since = "0.4.23", note = "use `Local::now()` instead")]
    #[allow(deprecated)]
    #[must_use]
    pub fn today() -> Date<Local> {
        Local::now().date()
    }

    /// Returns a `DateTime<Local>` which corresponds to the current date, time and offset from
    /// UTC.
    pub fn now() -> DateTime<Local> {
        Utc::now().with_timezone(&Local)
    }
}

impl TimeZone for Local {
    type Offset = FixedOffset;

    fn from_offset(_offset: &FixedOffset) -> Local {
        Local
    }

    #[allow(deprecated)]
    fn offset_from_local_date(&self, local: &NaiveDate) -> MappedLocalTime<FixedOffset> {
        // Get the offset at local midnight.
        self.offset_from_local_datetime(&local.and_time(NaiveTime::MIN))
    }

    fn offset_from_local_datetime(&self, local: &NaiveDateTime) -> MappedLocalTime<FixedOffset> {
        inner::offset_from_local_datetime(local)
    }

    #[allow(deprecated)]
    fn offset_from_utc_date(&self, utc: &NaiveDate) -> FixedOffset {
        // Get the offset at midnight.
        self.offset_from_utc_datetime(&utc.and_time(NaiveTime::MIN))
    }

    fn offset_from_utc_datetime(&self, utc: &NaiveDateTime) -> FixedOffset {
        inner::offset_from_utc_datetime(utc).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::{inner, Local};
    use crate::offset::TimeZone;
    use crate::{Datelike, Days, FixedOffset, MappedLocalTime, NaiveDate, Utc};

    #[test]
    fn verify_correct_offsets() {
        let now = Local::now();
        let from_local = Local.from_local_datetime(&now.naive_local()).unwrap();
        let from_utc = Local.from_utc_datetime(&now.naive_utc());

        assert_eq!(now.offset().local_minus_utc(), from_local.offset().local_minus_utc());
        assert_eq!(now.offset().local_minus_utc(), from_utc.offset().local_minus_utc());

        assert_eq!(now, from_local);
        assert_eq!(now, from_utc);
    }

    #[test]
    fn verify_correct_offsets_distant_past() {
        let distant_past = Local::now() - Days::new(365 * 500);
        let from_local = Local.from_local_datetime(&distant_past.naive_local()).unwrap();
        let from_utc = Local.from_utc_datetime(&distant_past.naive_utc());

        assert_eq!(distant_past.offset().local_minus_utc(), from_local.offset().local_minus_utc());
        assert_eq!(distant_past.offset().local_minus_utc(), from_utc.offset().local_minus_utc());

        assert_eq!(distant_past, from_local);
        assert_eq!(distant_past, from_utc);
    }

    #[test]
    fn verify_correct_offsets_distant_future() {
        let distant_future = Local::now() + Days::new(365 * 35000);
        let from_local = Local.from_local_datetime(&distant_future.naive_local()).unwrap();
        let from_utc = Local.from_utc_datetime(&distant_future.naive_utc());

        assert_eq!(
            distant_future.offset().local_minus_utc(),
            from_local.offset().local_minus_utc()
        );
        assert_eq!(distant_future.offset().local_minus_utc(), from_utc.offset().local_minus_utc());

        assert_eq!(distant_future, from_local);
        assert_eq!(distant_future, from_utc);
    }

    #[test]
    fn test_local_date_sanity_check() {
        assert_eq!(Local.with_ymd_and_hms(2999, 12, 28, 0, 0, 0).unwrap().day(), 28);
    }

    #[test]
    fn test_leap_second() {
        let today = Utc::now().date_naive();

        if let Some(dt) = today.and_hms_milli_opt(15, 2, 59, 1000) {
            let timestr = dt.time().to_string();
            assert!(
                timestr == "15:02:60" || timestr == "15:03:00",
                "unexpected timestr {timestr:?}"
            );
        }

        if let Some(dt) = today.and_hms_milli_opt(15, 2, 3, 1234) {
            let timestr = dt.time().to_string();
            assert!(
                timestr == "15:02:03.234" || timestr == "15:02:04.234",
                "unexpected timestr {timestr:?}"
            );
        }
    }

    #[test]
    #[cfg(all(
        feature = "clock",
        not(all(
            target_arch = "wasm32",
            not(any(target_os = "emscripten", target_os = "wasi"))
        ))
    ))]
    fn jiff_dst_gap_fold_and_unambiguous_mapping() {
        let Ok(ny) = jiff::tz::TimeZone::get("America/New_York") else {
            return;
        };
        let est = FixedOffset::west_opt(5 * 3600).unwrap();
        let edt = FixedOffset::west_opt(4 * 3600).unwrap();

        let gap_dt = NaiveDate::from_ymd_opt(2024, 3, 10).unwrap().and_hms_opt(2, 30, 0).unwrap();
        assert_eq!(inner::offset_from_local_datetime_in_tz(&ny, &gap_dt), MappedLocalTime::None);

        let fold_dt = NaiveDate::from_ymd_opt(2024, 11, 3).unwrap().and_hms_opt(1, 30, 0).unwrap();
        assert_eq!(
            inner::offset_from_local_datetime_in_tz(&ny, &fold_dt),
            MappedLocalTime::Ambiguous(edt, est)
        );

        let summer_dt =
            NaiveDate::from_ymd_opt(2024, 7, 15).unwrap().and_hms_opt(17, 30, 0).unwrap();
        assert_eq!(
            inner::offset_from_local_datetime_in_tz(&ny, &summer_dt),
            MappedLocalTime::Single(edt)
        );
    }

    #[test]
    #[cfg(feature = "rkyv-validation")]
    fn test_rkyv_validation() {
        let local = Local;
        let bytes = rkyv::to_bytes::<_, 0>(&local).unwrap();
        assert_eq!(bytes.len(), 0);
        assert_eq!(rkyv::from_bytes::<Local>(&bytes).unwrap(), super::ArchivedLocal);
    }
}
