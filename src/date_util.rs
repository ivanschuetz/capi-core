use std::convert::TryInto;

use anyhow::Result;
use chrono::{DateTime, NaiveDateTime, Utc};

pub fn timestamp_seconds_to_date(timestamp: u64) -> Result<DateTime<Utc>> {
    // i64::MAX is in the year 2262, where if this program still exists and is regularly updated, the dependencies should require suitable types.
    // until then we don't expect this to fail (under normal circumstances).
    let timestamp_i64 = timestamp.try_into()?;
    Ok(DateTime::<Utc>::from_utc(
        NaiveDateTime::from_timestamp(timestamp_i64, 0),
        Utc,
    ))
}
