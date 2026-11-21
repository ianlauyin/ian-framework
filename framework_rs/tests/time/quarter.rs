use chrono::NaiveDate;

use ian_framework_rs::time::quarter::{QUARTER_NUMBER_INVALID, Quarter};

fn day(year: i32, month: u32, date: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, date).unwrap()
}

fn quarter(year: i32, number: u32) -> Quarter {
    Quarter::new(year, number).unwrap()
}

#[test]
fn a_day_belongs_to_the_quarter_of_its_month() {
    assert_eq!(Quarter::of(day(2026, 1, 1)), quarter(2026, 1));
    assert_eq!(Quarter::of(day(2026, 3, 31)), quarter(2026, 1));
    assert_eq!(Quarter::of(day(2026, 4, 1)), quarter(2026, 2));
    assert_eq!(Quarter::of(day(2026, 9, 30)), quarter(2026, 3));
    assert_eq!(Quarter::of(day(2026, 12, 31)), quarter(2026, 4));
}

#[test]
fn the_bounds_are_text_days_of_the_first_and_last_month() {
    let value = quarter(2026, 4);
    assert_eq!(value.first_day(), "2026-10-01");
    assert_eq!(value.last_day(), "2026-12-31");
}

#[test]
fn a_number_outside_1_to_4_is_refused() {
    for number in [0, 5] {
        let error = Quarter::new(2026, number).unwrap_err();
        assert_eq!(error.code(), QUARTER_NUMBER_INVALID);
        assert_eq!(error.message(), "quarter number must be from 1 to 4");
    }
}

#[test]
fn the_last_day_is_a_real_date_in_each_quarter() {
    let last_days: Vec<String> = (1..=4)
        .map(|number| quarter(2026, number).last_day())
        .collect();
    assert_eq!(
        last_days,
        ["2026-03-31", "2026-06-30", "2026-09-30", "2026-12-31"]
    );
}

#[test]
fn a_quarter_shows_as_year_and_number() {
    assert_eq!(quarter(2026, 2).to_string(), "2026-Q2");
}

#[test]
fn an_earlier_quarter_is_less_than_a_later_one() {
    assert!(quarter(2025, 4) < quarter(2026, 1));
}
