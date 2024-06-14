use std::str::FromStr;

use chrono::{DateTime, Datelike, Days, NaiveDate, NaiveDateTime, NaiveTime, TimeDelta, Weekday};
use chrono_tz::Tz;

use thiserror::Error;

fn main() {
    println!("Hello, world!");
}

struct Shift {
    start: DateTime<Tz>,
    duration: TimeDelta,
}

struct WeeklyShiftPattern {
    shifts: Vec<WeeklyShift>,
}

impl FromStr for WeeklyShiftPattern {
    type Err = WeeklyShiftParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parsed_shifts: Result<Vec<WeeklyShift>, Self::Err> = s
            .lines()
            .map(|line| Ok(line.parse::<WeeklyShift>()?))
            .collect();
        Ok(WeeklyShiftPattern {
            shifts: parsed_shifts?,
        })
    }
}

struct WeeklyShift {
    weekday: Weekday,
    start: NaiveTime,
    start_tz: Tz,
}

impl WeeklyShift {
    fn prev_start(&self, dt: &DateTime<Tz>) -> DateTime<Tz> {
        let date_in_shift_tz = dt.with_timezone(&self.start_tz).date_naive();
        let current_week_shift_start_date = NaiveDate::from_isoywd_opt(
            date_in_shift_tz.iso_week().year(),
            date_in_shift_tz.iso_week().week(),
            self.weekday,
        )
        .unwrap();

        let current_week_shift_start = current_week_shift_start_date
            .and_time(self.start)
            .and_local_timezone(self.start_tz)
            .earliest()
            .unwrap();
        if &current_week_shift_start <= dt {
            current_week_shift_start
        } else {
            current_week_shift_start_date
                .checked_sub_days(Days::new(7))
                .unwrap()
                .and_time(self.start)
                .and_local_timezone(self.start_tz)
                .earliest()
                .unwrap()
        }
    }
}

#[derive(Error, Debug)]
pub enum WeeklyShiftParseError {
    #[error("Cannot parse weekday field")]
    Weekday(#[from] chrono::ParseWeekdayError),
    #[error("Cannot parse start field")]
    StartTime(#[from] chrono::ParseError),
    #[error("Cannot parse timezone field")]
    TimeZone(#[from] chrono_tz::ParseError),
    #[error("Cannot read shift field")]
    InvalidWeeklyShift,
}

impl FromStr for WeeklyShift {
    type Err = WeeklyShiftParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut split_str = s.split(' ');
        let weekday: Weekday = if let Some(weekday_str) = split_str.next() {
            weekday_str.parse()?
        } else {
            return Err(WeeklyShiftParseError::InvalidWeeklyShift);
        };
        let start: NaiveTime = if let Some(start_str) = split_str.next() {
            start_str.parse()?
        } else {
            return Err(WeeklyShiftParseError::InvalidWeeklyShift);
        };
        let start_tz: Tz = if let Some(start_tz_str) = split_str.next() {
            start_tz_str.parse()?
        } else {
            return Err(WeeklyShiftParseError::InvalidWeeklyShift);
        };
        Ok(WeeklyShift {
            weekday,
            start,
            start_tz,
        })
    }
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, Offset, TimeZone, Timelike};

    use super::*;

    #[test]
    fn test_parse_shift_str() {
        let s = "Monday 12:30 Europe/London";
        let weekly_shift: WeeklyShift = s.parse().unwrap();
        assert_eq!(weekly_shift.weekday, Weekday::Mon);
        assert_eq!(weekly_shift.start.hour(), 12);
        assert_eq!(weekly_shift.start.minute(), 30);
        let first_jan = NaiveDate::from_ymd_opt(2020, 1, 1).unwrap();
        let first_jun = NaiveDate::from_ymd_opt(2020, 6, 1).unwrap();

        // Zero offset with no DST
        assert_eq!(
            weekly_shift
                .start_tz
                .offset_from_utc_date(&first_jan)
                .fix()
                .local_minus_utc(),
            0
        );
        // One hour offset with DST
        assert_eq!(
            weekly_shift
                .start_tz
                .offset_from_utc_date(&first_jun)
                .fix()
                .local_minus_utc(),
            3600
        );
    }

    #[test]
    fn test_prev_shift_start() {
        let shift: WeeklyShift = "Monday 12:30 Etc/UTC".parse().unwrap();
        let expected_dt = DateTime::parse_from_rfc3339("2000-01-03T12:30:00+00:00").unwrap();

        let success_dts = vec![
            NaiveDate::from_ymd_opt(2000, 1, 3)
                .unwrap()
                .and_hms_opt(12, 30, 0)
                .unwrap()
                .and_local_timezone(chrono_tz::UTC)
                .earliest()
                .unwrap(),
            NaiveDate::from_ymd_opt(2000, 1, 10)
                .unwrap()
                .and_hms_opt(12, 29, 59)
                .unwrap()
                .and_local_timezone(chrono_tz::UTC)
                .earliest()
                .unwrap(),
        ];

        let failure_dts = vec![
            // Timezone shifts out of range
            NaiveDate::from_ymd_opt(2000, 1, 3)
                .unwrap()
                .and_hms_opt(12, 30, 0)
                .unwrap()
                .and_local_timezone(chrono_tz::CET)
                .earliest()
                .unwrap(),
            NaiveDate::from_ymd_opt(2000, 1, 10)
                .unwrap()
                .and_hms_opt(12, 29, 59)
                .unwrap()
                .and_local_timezone(chrono_tz::EST)
                .earliest()
                .unwrap(),
            // A day too early or late
            NaiveDate::from_ymd_opt(2000, 1, 2)
                .unwrap()
                .and_hms_opt(12, 30, 0)
                .unwrap()
                .and_local_timezone(chrono_tz::UTC)
                .earliest()
                .unwrap(),
            NaiveDate::from_ymd_opt(2000, 1, 11)
                .unwrap()
                .and_hms_opt(12, 29, 59)
                .unwrap()
                .and_local_timezone(chrono_tz::UTC)
                .earliest()
                .unwrap(),
            // A second too early or late
            NaiveDate::from_ymd_opt(2000, 1, 3)
                .unwrap()
                .and_hms_opt(12, 29, 59)
                .unwrap()
                .and_local_timezone(chrono_tz::UTC)
                .earliest()
                .unwrap(),
            NaiveDate::from_ymd_opt(2000, 1, 10)
                .unwrap()
                .and_hms_opt(12, 30, 00)
                .unwrap()
                .and_local_timezone(chrono_tz::UTC)
                .earliest()
                .unwrap(),
        ];

        for trial_dt in success_dts.iter() {
            assert_eq!(shift.prev_start(trial_dt), expected_dt)
        }
        for trial_dt in failure_dts.iter() {
            assert_ne!(shift.prev_start(trial_dt), expected_dt)
        }
    }

    #[test]
    fn test_shift_from_datetime() {
        let shifts = "Monday 12:00 Europe/London
Wednesday 00:00 Europe/London
Saturday 08:00 Europe/London";
    }
}
