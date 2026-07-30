use icu_calendar::{
    AnyCalendar, AnyCalendarKind, Date, Iso,
    options::{
        DateAddOptions, DateDifferenceOptions, DateDurationUnit, DateFromFieldsOptions, Overflow,
    },
    types::{DateDuration, DateFields, Month},
};

use super::IsoDate;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarDate {
    pub year: i32,
    pub month: u8,
    pub month_code: String,
    pub day: u8,
    pub era: Option<String>,
    pub era_year: Option<i32>,
    pub day_of_year: u16,
    pub days_in_month: u8,
    pub days_in_year: u16,
    pub months_in_year: u8,
    pub in_leap_year: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarDateFields {
    pub year: i32,
    pub month: Option<u8>,
    pub month_code: Option<String>,
    pub day: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CalendarDuration {
    pub years: i32,
    pub months: i32,
    pub weeks: i32,
    pub days: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalendarOverflow {
    Constrain,
    Reject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalendarLargestUnit {
    Year,
    Month,
    Week,
    Day,
}

pub trait CalendarBackend {
    fn date_from_iso(&self, calendar: &str, iso: IsoDate) -> Result<CalendarDate, String>;
    fn date_to_iso(
        &self,
        calendar: &str,
        year: i32,
        month_code: &str,
        day: u8,
    ) -> Result<IsoDate, String>;
    fn resolve_date_fields(
        &self,
        calendar: &str,
        fields: &CalendarDateFields,
    ) -> Result<IsoDate, String>;
    fn add_date(
        &self,
        calendar: &str,
        iso: IsoDate,
        duration: CalendarDuration,
        overflow: CalendarOverflow,
    ) -> Result<IsoDate, String>;
    fn date_until(
        &self,
        calendar: &str,
        start: IsoDate,
        end: IsoDate,
        largest_unit: CalendarLargestUnit,
    ) -> Result<CalendarDuration, String>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Icu4xCalendarBackend;

fn calendar_kind(calendar: &str) -> Result<AnyCalendarKind, String> {
    Ok(match calendar {
        "buddhist" => AnyCalendarKind::Buddhist,
        "chinese" => AnyCalendarKind::Chinese,
        "coptic" => AnyCalendarKind::Coptic,
        "dangi" => AnyCalendarKind::Dangi,
        "ethioaa" => AnyCalendarKind::EthiopianAmeteAlem,
        "ethiopic" => AnyCalendarKind::Ethiopian,
        "gregory" => AnyCalendarKind::Gregorian,
        "hebrew" => AnyCalendarKind::Hebrew,
        "indian" => AnyCalendarKind::Indian,
        "islamic" | "islamic-rgsa" | "islamic-umalqura" => AnyCalendarKind::HijriUmmAlQura,
        "islamic-civil" => AnyCalendarKind::HijriTabularTypeIIFriday,
        "islamic-tbla" => AnyCalendarKind::HijriTabularTypeIIThursday,
        "iso8601" => AnyCalendarKind::Iso,
        "japanese" => AnyCalendarKind::Japanese,
        "persian" => AnyCalendarKind::Persian,
        "roc" => AnyCalendarKind::Roc,
        _ => return Err(format!("unsupported calendar `{calendar}`")),
    })
}

fn iso_day_number(date: IsoDate) -> i64 {
    let mut year = date.year;
    let month = date.month as i32;
    year -= i32::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let shifted_month = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * shifted_month + 2) / 5 + date.day as i32 - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era as i64 * 146_097 + day_of_era as i64
}

impl CalendarBackend for Icu4xCalendarBackend {
    fn date_from_iso(&self, calendar: &str, iso: IsoDate) -> Result<CalendarDate, String> {
        let iso =
            Date::try_new_iso(iso.year, iso.month, iso.day).map_err(|error| error.to_string())?;
        let date = iso.to_calendar(AnyCalendar::new(calendar_kind(calendar)?));
        let year = date.year();
        let era = year.era();
        Ok(CalendarDate {
            year: year.extended_year(),
            month: date.month().ordinal,
            month_code: date.month().to_input().code().to_string(),
            day: date.day_of_month().0,
            era: era.map(|value| value.era.to_string()),
            era_year: era.map(|value| value.year),
            day_of_year: date.day_of_year().0,
            days_in_month: date.days_in_month(),
            days_in_year: date.days_in_year(),
            months_in_year: date.months_in_year(),
            in_leap_year: date.is_in_leap_year(),
        })
    }

    fn date_to_iso(
        &self,
        calendar: &str,
        year: i32,
        month_code: &str,
        day: u8,
    ) -> Result<IsoDate, String> {
        let leap = month_code.ends_with('L');
        let digits = month_code
            .strip_prefix('M')
            .and_then(|value| value.strip_suffix('L').or(Some(value)))
            .ok_or_else(|| format!("invalid month code `{month_code}`"))?;
        let number = digits
            .parse::<u8>()
            .map_err(|_| format!("invalid month code `{month_code}`"))?;
        if !(1..=99).contains(&number) {
            return Err(format!("invalid month code `{month_code}`"));
        }
        let month = if leap {
            Month::leap(number)
        } else {
            Month::new(number)
        };
        let date = Date::try_new(
            year.into(),
            month,
            day,
            AnyCalendar::new(calendar_kind(calendar)?),
        )
        .map_err(|error| error.to_string())?
        .to_calendar(Iso);
        Ok(IsoDate {
            year: date.year().extended_year(),
            month: date.month().number(),
            day: date.day_of_month().0,
        })
    }

    fn resolve_date_fields(
        &self,
        calendar: &str,
        fields: &CalendarDateFields,
    ) -> Result<IsoDate, String> {
        if fields.month.is_none() && fields.month_code.is_none() {
            return Err("calendar date requires month or monthCode".to_string());
        }
        let mut date_fields = DateFields::default();
        date_fields.extended_year = Some(fields.year);
        date_fields.ordinal_month = fields.month;
        date_fields.month_code = fields.month_code.as_deref().map(str::as_bytes);
        date_fields.day = Some(fields.day);
        let date = Date::try_from_fields(
            date_fields,
            DateFromFieldsOptions::default(),
            AnyCalendar::new(calendar_kind(calendar)?),
        )
        .map_err(|error| error.to_string())?
        .to_calendar(Iso);
        Ok(IsoDate {
            year: date.year().extended_year(),
            month: date.month().number(),
            day: date.day_of_month().0,
        })
    }

    fn add_date(
        &self,
        calendar: &str,
        iso: IsoDate,
        duration: CalendarDuration,
        overflow: CalendarOverflow,
    ) -> Result<IsoDate, String> {
        let values = [
            duration.years,
            duration.months,
            duration.weeks,
            duration.days,
        ];
        let is_negative = values.iter().any(|value| *value < 0);
        if values
            .iter()
            .any(|value| *value != 0 && (*value < 0) != is_negative)
        {
            return Err("calendar duration fields must have the same sign".to_string());
        }

        let date = Date::try_new_iso(iso.year, iso.month, iso.day)
            .map_err(|error| error.to_string())?
            .to_calendar(AnyCalendar::new(calendar_kind(calendar)?));
        let mut options = DateAddOptions::default();
        options.overflow = Some(match overflow {
            CalendarOverflow::Constrain => Overflow::Constrain,
            CalendarOverflow::Reject => Overflow::Reject,
        });
        let result = date
            .try_added_with_options(
                DateDuration {
                    is_negative,
                    years: duration.years.unsigned_abs(),
                    months: duration.months.unsigned_abs(),
                    weeks: duration.weeks.unsigned_abs(),
                    days: duration.days.unsigned_abs(),
                },
                options,
            )
            .map_err(|error| error.to_string())?
            .to_calendar(Iso);
        Ok(IsoDate {
            year: result.year().extended_year(),
            month: result.month().number(),
            day: result.day_of_month().0,
        })
    }

    fn date_until(
        &self,
        calendar: &str,
        start: IsoDate,
        end: IsoDate,
        largest_unit: CalendarLargestUnit,
    ) -> Result<CalendarDuration, String> {
        if matches!(
            largest_unit,
            CalendarLargestUnit::Week | CalendarLargestUnit::Day
        ) {
            let days = iso_day_number(end) - iso_day_number(start);
            return Ok(if largest_unit == CalendarLargestUnit::Week {
                CalendarDuration {
                    weeks: (days / 7) as i32,
                    days: (days % 7) as i32,
                    ..CalendarDuration::default()
                }
            } else {
                CalendarDuration {
                    days: days as i32,
                    ..CalendarDuration::default()
                }
            });
        }
        let calendar = AnyCalendar::new(calendar_kind(calendar)?);
        let start = Date::try_new_iso(start.year, start.month, start.day)
            .map_err(|error| error.to_string())?
            .to_calendar(calendar.clone());
        let end = Date::try_new_iso(end.year, end.month, end.day)
            .map_err(|error| error.to_string())?
            .to_calendar(calendar);
        let mut options = DateDifferenceOptions::default();
        options.largest_unit = Some(match largest_unit {
            CalendarLargestUnit::Year => DateDurationUnit::Years,
            CalendarLargestUnit::Month => DateDurationUnit::Months,
            CalendarLargestUnit::Week => DateDurationUnit::Weeks,
            CalendarLargestUnit::Day => DateDurationUnit::Days,
        });
        let result = start
            .try_until_with_options(&end, options)
            .map_err(|error| error.to_string())?;
        let sign = if result.is_negative { -1 } else { 1 };
        Ok(CalendarDuration {
            years: sign * result.years as i32,
            months: sign * result.months as i32,
            weeks: sign * result.weeks as i32,
            days: sign * result.days as i32,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_iso_to_complex_calendars_and_back() {
        let backend = Icu4xCalendarBackend;
        let iso = IsoDate {
            year: 2024,
            month: 2,
            day: 10,
        };
        for calendar in ["chinese", "dangi", "hebrew", "persian", "islamic-umalqura"] {
            let converted = backend.date_from_iso(calendar, iso).unwrap();
            let round_trip = backend
                .date_to_iso(
                    calendar,
                    converted.year,
                    &converted.month_code,
                    converted.day,
                )
                .unwrap();
            assert_eq!(round_trip, iso, "{calendar}");
        }
    }

    #[test]
    fn exposes_temporal_month_codes_for_leap_months() {
        let converted = Icu4xCalendarBackend
            .date_from_iso(
                "chinese",
                IsoDate {
                    year: 2023,
                    month: 3,
                    day: 22,
                },
            )
            .unwrap();
        assert!(converted.month_code.starts_with('M'));
    }

    #[test]
    fn adds_and_differences_dates_in_calendar_space() {
        let backend = Icu4xCalendarBackend;
        let start = IsoDate {
            year: 2023,
            month: 3,
            day: 22,
        };
        let end = backend
            .add_date(
                "chinese",
                start,
                CalendarDuration {
                    months: 1,
                    ..CalendarDuration::default()
                },
                CalendarOverflow::Constrain,
            )
            .unwrap();
        assert_eq!(
            backend
                .date_until("chinese", start, end, CalendarLargestUnit::Month)
                .unwrap(),
            CalendarDuration {
                months: 1,
                ..CalendarDuration::default()
            }
        );
    }

    #[test]
    fn week_and_day_differences_use_fixed_iso_days() {
        let backend = Icu4xCalendarBackend;
        let start = IsoDate {
            year: 2023,
            month: 3,
            day: 22,
        };
        let end = IsoDate {
            year: 2024,
            month: 3,
            day: 22,
        };
        assert_eq!(
            backend
                .date_until("chinese", start, end, CalendarLargestUnit::Week)
                .unwrap(),
            CalendarDuration {
                weeks: 52,
                days: 2,
                ..CalendarDuration::default()
            }
        );
        assert_eq!(
            backend
                .date_until("hebrew", end, start, CalendarLargestUnit::Day)
                .unwrap()
                .days,
            -366
        );
    }

    #[test]
    fn resolves_leap_month_fields_without_losing_month_code() {
        let backend = Icu4xCalendarBackend;
        let iso = backend
            .resolve_date_fields(
                "chinese",
                &CalendarDateFields {
                    year: 2023,
                    month: None,
                    month_code: Some("M02L".into()),
                    day: 1,
                },
            )
            .unwrap();
        let resolved = backend.date_from_iso("chinese", iso).unwrap();
        assert_eq!(resolved.month_code, "M02L");
    }
}
