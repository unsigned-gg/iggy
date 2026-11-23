use std::error::Error as StdError;
use std::fmt;
use std::time::Duration;

/// Error parsing human-friendly duration
#[derive(Debug, PartialEq, Clone)]
pub enum Error {
    /// Invalid character during parsing
    ///
    /// More specifically anything that is not alphanumeric is prohibited
    ///
    /// The field is an byte offset of the character in the string.
    InvalidCharacter(usize),
    /// Non-numeric value where number is expected
    ///
    /// This usually means that either time unit is broken into words,
    /// e.g. `m sec` instead of `msec`, or just number is omitted,
    /// for example `2 hours min` instead of `2 hours 1 min`
    ///
    /// The field is an byte offset of the errorneous character
    /// in the string.
    NumberExpected(usize),
    /// Unit in the number is not one of allowed units
    ///
    /// See documentation of `parse_duration` for the list of supported
    /// time units.
    ///
    /// The two fields are start and end (exclusive) of the slice from
    /// the original string, containing errorneous value
    UnknownUnit {
        /// Start of the invalid unit inside the original string
        start: usize,
        /// End of the invalid unit inside the original string
        end: usize,
        /// The unit verbatim
        unit: String,
        /// A number associated with the unit
        value: u64,
    },
    /// The numeric value exceeds the limits of this library.
    ///
    /// This can mean two things:
    /// - The value is too large to be useful.
    /// - The attempted precision is not supported.
    ///   For instance, a duration of `0.5ns` is not supported,
    ///   because durations below one nanosecond cannot be represented.
    NumberOverflow,
    /// The value was an empty string (or consists only whitespace)
    Empty,
}

impl StdError for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InvalidCharacter(offset) => write!(f, "invalid character at {}", offset),
            Error::NumberExpected(offset) => write!(f, "expected number at {}", offset),
            Error::UnknownUnit { unit, value, .. } if unit.is_empty() => {
                write!(f, "time unit needed, for example {0}sec or {0}ms", value)
            }
            Error::UnknownUnit { unit, .. } => {
                write!(
                    f,
                    "unknown time unit {:?}, \
                    supported units: ns, us/µs, ms, sec, min, hours, days, \
                    weeks, months, years (and few variations)",
                    unit
                )
            }
            Error::NumberOverflow => write!(f, "number is too large or cannot be represented without a lack of precision (values below 1ns are not supported)"),
            Error::Empty => write!(f, "value was empty"),
        }
    }
}

/// A wrapper type that allows you to Display a Duration
#[derive(Debug, Clone)]
pub struct FormattedDuration(Duration);

const NANOS_PER_SEC: u64 = 1_000_000_000;
const MINUTE: u64 = 60;
const HOUR: u64 = 3600;
const DAY: u64 = 86_400;
const WEEK: u64 = 7 * DAY;
const MONTH: u64 = 2_630_016; // 30.44 days
const YEAR: u64 = 31_557_600; // 365.25 days

fn mul(a: u64, b: u64) -> Result<u64, Error> {
    a.checked_mul(b).ok_or(Error::NumberOverflow)
}

fn add(a: u64, b: u64) -> Result<u64, Error> {
    a.checked_add(b).ok_or(Error::NumberOverflow)
}

/// Divides exactly. A remainder means the value needs more precision than the
/// target unit has.
fn div_exact(a: u64, b: u64) -> Result<u64, Error> {
    if a % b == 0 {
        Ok(a / b)
    } else {
        Err(Error::NumberOverflow)
    }
}

fn push_digit(n: u64, c: char) -> Result<u64, Error> {
    add(mul(n, 10)?, c as u64 - '0' as u64)
}

fn is_unit_char(c: char) -> bool {
    c.is_ascii_alphabetic() || c == 'µ'
}

/// How many seconds, or how many nanoseconds, one of a unit is.
#[derive(Clone, Copy)]
enum Unit {
    Nanos(u64),
    Secs(u64),
}

fn unit(s: &str) -> Option<Unit> {
    Some(match s {
        "nanos" | "nsec" | "ns" => Unit::Nanos(1),
        "usec" | "us" | "µs" => Unit::Nanos(1_000),
        "millis" | "msec" | "ms" => Unit::Nanos(1_000_000),
        "seconds" | "second" | "secs" | "sec" | "s" => Unit::Secs(1),
        "minutes" | "minute" | "min" | "mins" | "m" => Unit::Secs(MINUTE),
        "hours" | "hour" | "hr" | "hrs" | "h" => Unit::Secs(HOUR),
        "days" | "day" | "d" => Unit::Secs(DAY),
        "weeks" | "week" | "wk" | "wks" | "w" => Unit::Secs(WEEK),
        "months" | "month" | "M" => Unit::Secs(MONTH),
        "years" | "year" | "yr" | "yrs" | "y" => Unit::Secs(YEAR),
        _ => return None,
    })
}

/// A cursor over the input that reports byte offsets.
struct Cursor<'a> {
    src: &'a str,
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn peek(&self) -> Option<char> {
        self.src[self.pos..].chars().next()
    }

    fn bump(&mut self, c: char) {
        self.pos += c.len_utf8();
    }

    /// Skips whitespace and takes the first digit of a number. `Ok(None)` at
    /// the end of the input. The error offset is where the skip started.
    fn first_digit(&mut self) -> Result<Option<u64>, Error> {
        let start = self.pos;
        while let Some(c) = self.peek() {
            self.bump(c);
            if c.is_ascii_digit() {
                return Ok(Some(c as u64 - '0' as u64));
            }
            if !c.is_whitespace() {
                return Err(Error::NumberExpected(start));
            }
        }
        Ok(None)
    }

    /// Takes the digits after a decimal point, up to the unit. Returns the
    /// fraction as (numerator, denominator). `dot` is the offset of the point.
    fn fraction(&mut self, dot: usize) -> Result<(u64, u64), Error> {
        let mut numerator = 0u64;
        let mut denominator = 1u64;
        // Offset reported for a bad character: the end of the last accepted
        // character, which is the point itself until a digit or a space
        // follows it. humantime 2.4 reports the same offsets.
        let mut at = dot;
        while let Some(c) = self.peek() {
            if is_unit_char(c) {
                break;
            }
            if c.is_ascii_digit() {
                denominator = mul(denominator, 10)?;
                numerator = push_digit(numerator, c)?;
            } else if !c.is_whitespace() {
                return Err(Error::InvalidCharacter(at));
            }
            self.bump(c);
            at = self.pos;
        }
        if denominator == 1 {
            // No digit after the point, as in "1.".
            return Err(Error::InvalidCharacter(at));
        }
        Ok((numerator, denominator))
    }
}

fn add_to(out: &mut Duration, mut sec: u64, nsec: u64) -> Result<(), Error> {
    let mut nsec = add(u64::from(out.subsec_nanos()), nsec)?;
    if nsec >= NANOS_PER_SEC {
        sec = add(sec, nsec / NANOS_PER_SEC)?;
        nsec %= NANOS_PER_SEC;
    }
    sec = add(out.as_secs(), sec)?;
    *out = Duration::new(sec, nsec as u32);
    Ok(())
}

fn add_span(
    out: &mut Duration,
    n: u64,
    frac: Option<(u64, u64)>,
    name: &str,
    start: usize,
    end: usize,
) -> Result<(), Error> {
    let Some(unit) = unit(name) else {
        return Err(Error::UnknownUnit {
            start,
            end,
            unit: name.to_owned(),
            value: n,
        });
    };
    match unit {
        Unit::Nanos(per) => add_to(out, 0, mul(n, per)?)?,
        Unit::Secs(per) => add_to(out, mul(n, per)?, 0)?,
    }
    if let Some((num, den)) = frac {
        match unit {
            // Nothing is smaller than a nanosecond.
            Unit::Nanos(1) => return Err(Error::NumberOverflow),
            Unit::Nanos(per) => add_to(out, 0, div_exact(mul(num, per)?, den)?)?,
            // Seconds and minutes keep a fraction down to the nanosecond.
            Unit::Secs(per) if per <= MINUTE => {
                add_to(out, 0, div_exact(mul(num, per * NANOS_PER_SEC)?, den)?)?
            }
            // Longer units keep a fraction down to the second.
            Unit::Secs(per) => add_to(out, div_exact(mul(num, per)?, den)?, 0)?,
        }
    }
    Ok(())
}

/// Parse duration object `1hour 12min 5s`
///
/// The duration object is a concatenation of time spans. Where each time
/// span is an integer number and a suffix. Supported suffixes:
///
/// * `nsec`, `ns` -- nanoseconds
/// * `usec`, `us`, `µs` -- microseconds
/// * `msec`, `ms` -- milliseconds
/// * `seconds`, `second`, `sec`, `s`
/// * `minutes`, `minute`, `min`, `m`
/// * `hours`, `hour`, `hr`, `hrs`, `h`
/// * `days`, `day`, `d`
/// * `weeks`, `week`, `wk`, `wks`, `w`
/// * `months`, `month`, `M` -- defined as 30.44 days
/// * `years`, `year`, `yr`, `yrs`, `y` -- defined as 365.25 days
///
/// # Examples
///
/// ```
/// use std::time::Duration;
/// use humantime::parse_duration;
///
/// assert_eq!(parse_duration("2h 37min"), Ok(Duration::new(9420, 0)));
/// assert_eq!(parse_duration("32ms"), Ok(Duration::new(0, 32_000_000)));
/// assert_eq!(parse_duration("4.2s"), Ok(Duration::new(4, 200_000_000)));
/// ```
pub fn parse_duration(s: &str) -> Result<Duration, Error> {
    if s == "0" {
        return Ok(Duration::ZERO);
    }
    let mut cur = Cursor { src: s, pos: 0 };
    let mut out = Duration::ZERO;
    let mut n = cur.first_digit()?.ok_or(Error::Empty)?;
    loop {
        // The rest of the number. Whitespace inside it is skipped.
        let mut frac = None;
        while let Some(c) = cur.peek() {
            if c.is_ascii_digit() {
                n = push_digit(n, c)?;
            } else if c == '.' {
                let dot = cur.pos;
                cur.bump(c);
                frac = Some(cur.fraction(dot)?);
                break;
            } else if is_unit_char(c) {
                break;
            } else if !c.is_whitespace() {
                return Err(Error::InvalidCharacter(cur.pos));
            }
            cur.bump(c);
        }

        // The unit: letters up to whitespace, a digit or the end.
        let start = cur.pos;
        let mut next = None;
        let mut end = s.len();
        while let Some(c) = cur.peek() {
            if c.is_ascii_digit() {
                // The next span starts here with no space before it.
                end = cur.pos;
                cur.bump(c);
                next = Some(c as u64 - '0' as u64);
                break;
            }
            if c.is_whitespace() {
                end = cur.pos;
                cur.bump(c);
                break;
            }
            if !is_unit_char(c) {
                return Err(Error::InvalidCharacter(cur.pos));
            }
            cur.bump(c);
        }
        add_span(&mut out, n, frac, &s[start..end], start, end)?;

        n = match next {
            Some(digit) => digit,
            None => match cur.first_digit()? {
                Some(digit) => digit,
                None => return Ok(out),
            },
        };
    }
}

/// Formats duration into a human-readable string
///
/// Note: this format is guaranteed to have same value when using
/// parse_duration, but we can change some details of the exact composition
/// of the value.
///
/// # Examples
///
/// ```
/// use std::time::Duration;
/// use humantime::format_duration;
///
/// let val1 = Duration::new(9420, 0);
/// assert_eq!(format_duration(val1).to_string(), "2h 37m");
/// let val2 = Duration::new(0, 32_000_000);
/// assert_eq!(format_duration(val2).to_string(), "32ms");
/// ```
pub fn format_duration(val: Duration) -> FormattedDuration {
    FormattedDuration(val)
}

impl FormattedDuration {
    /// Returns a reference to the [`Duration`][] that is being formatted.
    pub fn get_ref(&self) -> &Duration {
        &self.0
    }
}

impl fmt::Display for FormattedDuration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let secs = self.0.as_secs();
        let nanos = u64::from(self.0.subsec_nanos());
        if secs == 0 && nanos == 0 {
            return f.write_str("0s");
        }
        let in_year = secs % YEAR;
        let in_month = in_year % MONTH;
        let in_day = in_month % DAY;
        let micro = if cfg!(feature = "mu") { "µs" } else { "us" };
        // (value, name, takes a plural "s")
        let parts = [
            (secs / YEAR, "year", true),
            (in_year / MONTH, "month", true),
            (in_month / DAY, "day", true),
            (in_day / HOUR, "h", false),
            (in_day % HOUR / MINUTE, "m", false),
            (in_day % MINUTE, "s", false),
            (nanos / 1_000_000, "ms", false),
            (nanos / 1_000 % 1_000, micro, false),
            (nanos % 1_000, "ns", false),
        ];
        let mut started = false;
        for (value, name, plural) in parts {
            if value == 0 {
                continue;
            }
            if started {
                f.write_str(" ")?;
            }
            write!(f, "{value}{name}")?;
            if plural && value > 1 {
                f.write_str("s")?;
            }
            started = true;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(s: &str, secs: u64, nanos: u32) {
        assert_eq!(parse_duration(s), Ok(Duration::new(secs, nanos)), "{s:?}");
    }

    #[test]
    fn parses_the_humantime_grammar() {
        ok("0", 0, 0);
        ok("2h 37min", 9420, 0);
        ok("32ms", 0, 32_000_000);
        ok("4.2s", 4, 200_000_000);
        ok("1h 1m 1s", 3661, 0);
        ok("7 days", 604_800, 0);
        ok("1M", 2_630_016, 0);
        ok("1y", 31_557_600, 0);
        ok("1.5d", 129_600, 0);
        ok("17µs", 0, 17_000);
        ok("1 0s", 10, 0);
        ok("1s500ms", 1, 500_000_000);
    }

    #[test]
    fn refuses_bad_input_with_humantime_errors() {
        assert_eq!(parse_duration(""), Err(Error::Empty));
        assert_eq!(parse_duration("  "), Err(Error::Empty));
        assert_eq!(parse_duration("s"), Err(Error::NumberExpected(0)));
        assert_eq!(parse_duration("1-s"), Err(Error::InvalidCharacter(1)));
        assert_eq!(parse_duration("1.week"), Err(Error::InvalidCharacter(1)));
        assert_eq!(parse_duration("0.5ns"), Err(Error::NumberOverflow));
        assert_eq!(parse_duration("0.0000000001s"), Err(Error::NumberOverflow));
        assert_eq!(
            parse_duration("18446744073709551615y"),
            Err(Error::NumberOverflow)
        );
        assert_eq!(
            parse_duration("10"),
            Err(Error::UnknownUnit {
                start: 2,
                end: 2,
                unit: String::new(),
                value: 10
            })
        );
        assert_eq!(
            parse_duration("5 parsecs"),
            Err(Error::UnknownUnit {
                start: 2,
                end: 9,
                unit: "parsecs".to_owned(),
                value: 5
            })
        );
    }

    #[test]
    fn formats_like_humantime() {
        let f = |secs, nanos| format_duration(Duration::new(secs, nanos)).to_string();
        assert_eq!(f(0, 0), "0s");
        assert_eq!(f(9420, 0), "2h 37m");
        assert_eq!(f(0, 32_000_000), "32ms");
        assert_eq!(f(31_557_600 * 2 + 2_630_016 + 86_400 * 3, 0), "2years 1month 3days");
        assert_eq!(f(u64::from(u32::MAX), 0), "136years 1month 5days 19h 54m 39s");
        assert_eq!(f(1, 1_001_001), "1s 1ms 1us 1ns");
    }
}
