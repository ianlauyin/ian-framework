use chrono::{DateTime, FixedOffset, NaiveDate, Utc};

const HKT_OFFSET_SECS: i32 = 8 * 3600;

pub fn hkt_now() -> DateTime<FixedOffset> {
    let now = Utc::now();
    match FixedOffset::east_opt(HKT_OFFSET_SECS) {
        Some(hkt) => now.with_timezone(&hkt),
        None => now.fixed_offset(),
    }
}

pub fn hkt_today() -> NaiveDate {
    hkt_now().date_naive()
}
