use chrono::{Datelike, NaiveDate};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Quarter {
    pub year: i32,
    pub number: u32,
}

impl Quarter {
    pub fn of(day: NaiveDate) -> Self {
        Self {
            year: day.year(),
            number: (day.month() - 1) / 3 + 1,
        }
    }

    // The stored day is text as YYYY-MM-DD, so the bounds are text too.
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
