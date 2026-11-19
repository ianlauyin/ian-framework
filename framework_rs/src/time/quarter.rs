use chrono::{Datelike, NaiveDate};

use crate::exception;
use crate::exception::Exception;

pub const QUARTER_NUMBER_INVALID: &str = "QUARTER_NUMBER_INVALID";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Quarter {
    year: i32,
    number: u32,
}

impl Quarter {
    pub fn new(year: i32, number: u32) -> Result<Self, Exception> {
        if !(1..=4).contains(&number) {
            return Err(exception!(
                code = QUARTER_NUMBER_INVALID,
                "quarter number must be from 1 to 4"
            ));
        }
        Ok(Self { year, number })
    }

    pub fn of(day: NaiveDate) -> Self {
        Self {
            year: day.year(),
            number: (day.month() - 1) / 3 + 1,
        }
    }

    pub fn first_day(self) -> String {
        format!("{:04}-{:02}-01", self.year, (self.number - 1) * 3 + 1)
    }

    pub fn last_day(self) -> String {
        let last_day_of_month = match self.number {
            2 | 3 => 30,
            _ => 31,
        };
        format!(
            "{:04}-{:02}-{}",
            self.year,
            self.number * 3,
            last_day_of_month
        )
    }
}

impl std::fmt::Display for Quarter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{:04}-Q{}", self.year, self.number)
    }
}
