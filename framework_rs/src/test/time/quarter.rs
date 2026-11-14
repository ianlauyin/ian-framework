use chrono::NaiveDate;

use crate::time::quarter::Quarter;

fn day(year: i32, month: u32, date: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, date).unwrap()
}

#[test]
fn a_day_belongs_to_the_quarter_of_its_month() {
    assert_eq!(Quarter::of(day(2026, 1, 1)).number, 1);
    assert_eq!(Quarter::of(day(2026, 3, 31)).number, 1);
    assert_eq!(Quarter::of(day(2026, 4, 1)).number, 2);
    assert_eq!(Quarter::of(day(2026, 9, 30)).number, 3);
    assert_eq!(Quarter::of(day(2026, 12, 31)).number, 4);
}

#[test]
fn the_bounds_are_text_days_of_the_first_and_last_month() {
    let quarter = Quarter {
        year: 2026,
        number: 4,
    };
    assert_eq!(quarter.first_day(), "2026-10-01");
    assert_eq!(quarter.last_day(), "2026-12-31");
}

#[test]
fn the_last_day_is_a_real_date_in_each_quarter() {
    let last_days: Vec<String> = (1..=4)
        .map(|number| Quarter { year: 2026, number }.last_day())
        .collect();
    assert_eq!(
        last_days,
        ["2026-03-31", "2026-06-30", "2026-09-30", "2026-12-31"]
    );
}

#[test]
fn a_quarter_shows_as_year_and_number() {
    assert_eq!(
        Quarter {
            year: 2026,
            number: 2
        }
        .to_string(),
        "2026-Q2"
    );
}

#[test]
fn an_earlier_quarter_is_less_than_a_later_one() {
    assert!(
        Quarter {
            year: 2025,
            number: 4
        } < Quarter {
            year: 2026,
            number: 1
        }
    );
}
