use std::error::Error as StdError;
use std::fmt;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use jiff::civil::{Date, DateTime, Time};
use jiff::SignedDuration;

/// The Unix epoch as a civil datetime in UTC.
///
/// The arithmetic here uses civil datetimes, not `jiff::Timestamp`: a
/// `Timestamp` stops at 9999-12-30T22:00:00Z and this API covers all of
/// year 9999.
fn epoch() -> DateTime {
    DateTime::constant(1970, 1, 1, 0, 0, 0, 0)
}

/// Error parsing datetime (timestamp)
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Error {
    /// Numeric component is out of range
    OutOfRange,
    /// Bad character where digit is expected
    InvalidDigit,
    /// Other formatting errors
    InvalidFormat,
}

impl StdError for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::OutOfRange => write!(f, "numeric component is out of range"),
            Error::InvalidDigit => write!(f, "bad character where digit is expected"),
            Error::InvalidFormat => write!(f, "timestamp format is invalid"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Precision {
    Smart,
    Seconds,
    Millis,
    Micros,
    Nanos,
}

/// A wrapper type that allows you to Display a SystemTime
#[derive(Debug, Clone)]
pub struct Rfc3339Timestamp(SystemTime, Precision);

/// Reads the two ASCII digits at `i` and `i + 1`.
fn two_digits(b: &[u8], i: usize) -> Result<i16, Error> {
    let hi = (b[i] as char).to_digit(10).ok_or(Error::InvalidDigit)?;
    let lo = (b[i + 1] as char).to_digit(10).ok_or(Error::InvalidDigit)?;
    Ok((hi * 10 + lo) as i16)
}

/// Parse RFC3339 timestamp `2018-02-14T00:28:07Z`
///
/// Supported feature: any precision of fractional
/// digits `2018-02-14T00:28:07.133Z`.
///
/// Unsupported feature: localized timestamps. Only UTC is supported.
pub fn parse_rfc3339(s: &str) -> Result<SystemTime, Error> {
    if s.len() < "2018-02-14T00:28:07Z".len() {
        return Err(Error::InvalidFormat);
    }
    let b = s.as_bytes();
    if b[10] != b'T' || (b.last() != Some(&b'Z') && !s.ends_with("+00:00")) {
        return Err(Error::InvalidFormat);
    }
    parse_rfc3339_weak(s)
}

/// Parse RFC3339-like timestamp `2018-02-14 00:28:07`
///
/// Supported features:
///
/// 1. Any precision of fractional digits `2018-02-14 00:28:07.133`.
/// 2. Supports timestamp with or without either of `T`, `Z` or `+00:00`.
/// 3. Anything valid for [`parse_rfc3339`](parse_rfc3339) is
///    valid for this function
///
/// Unsupported feature: localized timestamps. Only UTC is supported, even if
/// this function is called without a `Z`.
pub fn parse_rfc3339_weak(s: &str) -> Result<SystemTime, Error> {
    if s.len() < "2018-02-14T00:28:07".len() {
        return Err(Error::InvalidFormat);
    }
    let b = s.as_bytes();
    if b[4] != b'-'
        || b[7] != b'-'
        || (b[10] != b'T' && b[10] != b' ')
        || b[13] != b':'
        || b[16] != b':'
    {
        return Err(Error::InvalidFormat);
    }
    let year = two_digits(b, 0)? * 100 + two_digits(b, 2)?;
    let month = two_digits(b, 5)?;
    let day = two_digits(b, 8)?;
    let hour = two_digits(b, 11)?;
    let minute = two_digits(b, 14)?;
    let mut second = two_digits(b, 17)?;

    if year < 1970 || hour > 23 || minute > 59 || second > 60 {
        return Err(Error::OutOfRange);
    }
    // A leap second is read as the last second of the minute.
    if second == 60 {
        second = 59;
    }
    let date = Date::new(year, month as i8, day as i8).map_err(|_| Error::OutOfRange)?;

    let mut nanos: u32 = 0;
    let mut mult: u32 = 100_000_000;
    if b.get(19) == Some(&b'.') {
        for idx in 20..b.len() {
            if b[idx] == b'Z' {
                if idx == b.len() - 1 {
                    break;
                }
                return Err(Error::InvalidDigit);
            } else if b[idx] == b'+' {
                // Start of "+00:00", which must be at the end.
                if idx + 6 == b.len() {
                    break;
                }
                return Err(Error::InvalidDigit);
            }
            // Digits past the ninth are checked and then dropped.
            nanos += mult * (b[idx] as char).to_digit(10).ok_or(Error::InvalidDigit)?;
            mult /= 10;
        }
    } else if b.len() != 19 && (b.len() > 25 || (b[19] != b'Z' && (&b[19..] != b"+00:00"))) {
        return Err(Error::InvalidFormat);
    }

    let time = Time::new(hour as i8, minute as i8, second as i8, nanos as i32)
        .map_err(|_| Error::OutOfRange)?;
    let since_epoch = DateTime::from_parts(date, time).duration_since(epoch());
    let since_epoch = Duration::try_from(since_epoch).map_err(|_| Error::OutOfRange)?;
    #[cfg(all(
        target_pointer_width = "32",
        not(target_os = "windows"),
        not(all(target_arch = "wasm32", not(target_os = "emscripten")))
    ))]
    if since_epoch.as_secs() > i32::MAX as u64 {
        return Err(Error::OutOfRange);
    }
    Ok(UNIX_EPOCH + since_epoch)
}

/// Format an RFC3339 timestamp `2018-02-14T00:28:07Z`
///
/// This function formats timestamp with smart precision: i.e. if it has no
/// fractional seconds, they aren't written at all. And up to nine digits if
/// they are.
///
/// The value is always UTC and ignores system timezone.
pub fn format_rfc3339(system_time: SystemTime) -> Rfc3339Timestamp {
    Rfc3339Timestamp(system_time, Precision::Smart)
}

/// Format an RFC3339 timestamp `2018-02-14T00:28:07Z`
///
/// This format always shows timestamp without fractional seconds.
///
/// The value is always UTC and ignores system timezone.
pub fn format_rfc3339_seconds(system_time: SystemTime) -> Rfc3339Timestamp {
    Rfc3339Timestamp(system_time, Precision::Seconds)
}

/// Format an RFC3339 timestamp `2018-02-14T00:28:07.000Z`
///
/// This format always shows milliseconds even if millisecond value is zero.
///
/// The value is always UTC and ignores system timezone.
pub fn format_rfc3339_millis(system_time: SystemTime) -> Rfc3339Timestamp {
    Rfc3339Timestamp(system_time, Precision::Millis)
}

/// Format an RFC3339 timestamp `2018-02-14T00:28:07.000000Z`
///
/// This format always shows microseconds even if microsecond value is zero.
///
/// The value is always UTC and ignores system timezone.
pub fn format_rfc3339_micros(system_time: SystemTime) -> Rfc3339Timestamp {
    Rfc3339Timestamp(system_time, Precision::Micros)
}

/// Format an RFC3339 timestamp `2018-02-14T00:28:07.000000000Z`
///
/// This format always shows nanoseconds even if nanosecond value is zero.
///
/// The value is always UTC and ignores system timezone.
pub fn format_rfc3339_nanos(system_time: SystemTime) -> Rfc3339Timestamp {
    Rfc3339Timestamp(system_time, Precision::Nanos)
}

impl Rfc3339Timestamp {
    /// Returns a reference to the [`SystemTime`][] that is being formatted.
    pub fn get_ref(&self) -> &SystemTime {
        &self.0
    }
}

impl fmt::Display for Rfc3339Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Fails for a time before the Unix epoch and for a time after year
        // 9999.
        let since_epoch = self.0.duration_since(UNIX_EPOCH).map_err(|_| fmt::Error)?;
        let since_epoch = SignedDuration::try_from(since_epoch).map_err(|_| fmt::Error)?;
        let dt = epoch().checked_add(since_epoch).map_err(|_| fmt::Error)?;
        write!(
            f,
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
            dt.year(),
            dt.month(),
            dt.day(),
            dt.hour(),
            dt.minute(),
            dt.second()
        )?;
        let nanos = dt.subsec_nanosecond();
        match self.1 {
            Precision::Seconds => {}
            Precision::Smart if nanos == 0 => {}
            Precision::Millis => write!(f, ".{:03}", nanos / 1_000_000)?,
            Precision::Micros => write!(f, ".{:06}", nanos / 1_000)?,
            Precision::Smart | Precision::Nanos => write!(f, ".{:09}", nanos)?,
        }
        f.write_str("Z")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_and_parses_rfc3339() {
        let t = UNIX_EPOCH + Duration::new(1_518_563_312, 123_000_000);
        assert_eq!(format_rfc3339_seconds(t).to_string(), "2018-02-13T23:08:32Z");
        assert_eq!(format_rfc3339_millis(t).to_string(), "2018-02-13T23:08:32.123Z");
        assert_eq!(format_rfc3339(t).to_string(), "2018-02-13T23:08:32.123000000Z");
        assert_eq!(parse_rfc3339("2018-02-13T23:08:32.123Z"), Ok(t));
        assert_eq!(parse_rfc3339("2018-02-13T23:08:32.123+00:00"), Ok(t));
        assert_eq!(parse_rfc3339_weak("2018-02-13 23:08:32.123"), Ok(t));
        assert_eq!(
            format_rfc3339(UNIX_EPOCH + Duration::new(253_402_300_799, 0)).to_string(),
            "9999-12-31T23:59:59Z"
        );
        assert_eq!(
            parse_rfc3339("9999-12-31T23:59:59.999999999Z"),
            Ok(UNIX_EPOCH + Duration::new(253_402_300_799, 999_999_999))
        );
        assert_eq!(parse_rfc3339("1970-01-01T00:00:00Z"), Ok(UNIX_EPOCH));
        assert_eq!(
            parse_rfc3339("2016-12-31T23:59:60Z"),
            Ok(UNIX_EPOCH + Duration::new(1_483_228_799, 0))
        );
        let after_9999 = UNIX_EPOCH + Duration::new(253_402_300_800, 0);
        assert!(std::fmt::write(&mut String::new(), format_args!("{}", format_rfc3339(after_9999))).is_err());
    }

    #[test]
    fn refuses_bad_timestamps() {
        assert_eq!(parse_rfc3339("2018-02-13 23:08:32Z"), Err(Error::InvalidFormat));
        assert_eq!(parse_rfc3339("2018-02-30T23:08:32Z"), Err(Error::OutOfRange));
        assert_eq!(parse_rfc3339("1969-12-31T23:59:59Z"), Err(Error::OutOfRange));
        assert_eq!(parse_rfc3339("2018-0x-13T23:08:32Z"), Err(Error::InvalidDigit));
        assert_eq!(parse_rfc3339("2018-02-13T23:08:32+01:00"), Err(Error::InvalidFormat));
    }
}
