//! Human-friendly time parser and formatter.
//!
//! This crate is a compatibility crate for the public API of `humantime` 2.4.
//! It exists because upstream `humantime` is unmaintained (RUSTSEC-2026-0542).
//! Calendar arithmetic for RFC 3339 timestamps is done by `jiff`. The duration
//! grammar (`15days 2min 2s`) is specific to `humantime` (a month is 30.44
//! days, a year is 365.25 days) and has no `jiff` equivalent, so it is
//! implemented here over `std::time::Duration`.
//!
//! Known difference from `humantime` 2.4: formatting a time before the Unix
//! epoch returns `fmt::Error`; upstream panics.

#![forbid(unsafe_code)]
#![warn(missing_debug_implementations, missing_docs, unreachable_pub)]

mod date;
mod duration;
mod wrapper;

pub use self::date::Rfc3339Timestamp;
pub use self::date::{
    format_rfc3339, format_rfc3339_micros, format_rfc3339_millis, format_rfc3339_nanos,
    format_rfc3339_seconds,
};
pub use self::date::{parse_rfc3339, parse_rfc3339_weak, Error as TimestampError};
pub use self::duration::{format_duration, FormattedDuration};
pub use self::duration::{parse_duration, Error as DurationError};
pub use self::wrapper::{Duration, Timestamp};
