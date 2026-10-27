use chrono::{DateTime, FixedOffset, Utc};

const HKT_OFFSET_SECS: i32 = 8 * 3600;

pub fn hkt_now() -> DateTime<FixedOffset> {
    let now = Utc::now();
    match FixedOffset::east_opt(HKT_OFFSET_SECS) {
        Some(hkt) => now.with_timezone(&hkt),
        None => now.fixed_offset(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hkt_now_has_the_hkt_offset() {
        let before = Utc::now();
        let now = hkt_now();
        let after = Utc::now();
        assert_eq!(now.offset().local_minus_utc(), 8 * 3600);
        assert!(now >= before && now <= after);
        assert!(now.to_rfc3339().ends_with("+08:00"));
    }
}
