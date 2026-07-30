#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IsoDate {
    pub year: i32,
    pub month: u8,
    pub day: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IsoTime {
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub millisecond: u16,
    pub microsecond: u16,
    pub nanosecond: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IsoDateTime {
    pub date: IsoDate,
    pub time: IsoTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntlDateTimeInput {
    EpochMilliseconds(i64),
    Instant {
        epoch_nanoseconds: i128,
    },
    PlainDate {
        iso_date: IsoDate,
        calendar: String,
    },
    PlainTime {
        time: IsoTime,
    },
    PlainDateTime {
        iso_date_time: IsoDateTime,
        calendar: String,
    },
    PlainYearMonth {
        iso_year: i32,
        iso_month: u8,
        reference_iso_day: u8,
        calendar: String,
    },
    PlainMonthDay {
        iso_month: u8,
        iso_day: u8,
        reference_iso_year: i32,
        calendar: String,
    },
    ZonedDateTime {
        epoch_nanoseconds: i128,
        time_zone: String,
        calendar: String,
    },
}
