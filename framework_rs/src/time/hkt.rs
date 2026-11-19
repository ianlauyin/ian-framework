use chrono::{DateTime, FixedOffset, NaiveDate, Utc};

const HKT_OFFSET_SECS: i32 = 8 * 3600;

const HKT_OFFSET: FixedOffset = match FixedOffset::east_opt(HKT_OFFSET_SECS) {
    Some(offset) => offset,
    None => panic!("the Hong Kong offset is out of range"),
};

pub fn hkt_now() -> DateTime<FixedOffset> {
    Utc::now().with_timezone(&HKT_OFFSET)
}

pub fn hkt_today() -> NaiveDate {
    hkt_now().date_naive()
}
