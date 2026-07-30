use jiff::{
    Timestamp,
    civil::DateTime,
    tz::{Offset, TimeZone},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalDateTime {
    pub year: i32,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub nanosecond: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeZoneDisambiguation {
    Compatible,
    Earlier,
    Later,
    Reject,
}

pub trait TimeZoneProvider {
    fn offset_nanoseconds(&self, identifier: &str, epoch_nanoseconds: i128)
    -> Result<i128, String>;
    fn local_to_epoch_nanoseconds(
        &self,
        identifier: &str,
        local: LocalDateTime,
        disambiguation: TimeZoneDisambiguation,
    ) -> Result<i128, String>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct JiffTimeZoneProvider;

fn fixed_offset(identifier: &str) -> Option<i32> {
    if identifier.eq_ignore_ascii_case("UTC") {
        return Some(0);
    }
    let sign = match identifier.as_bytes().first()? {
        b'+' => 1,
        b'-' => -1,
        _ => return None,
    };
    let mut parts = identifier[1..].split(':');
    let hours = parts.next()?.parse::<i32>().ok()?;
    let minutes = parts.next()?.parse::<i32>().ok()?;
    let seconds = parts
        .next()
        .map(str::parse::<i32>)
        .transpose()
        .ok()?
        .unwrap_or(0);
    if parts.next().is_some() || hours > 23 || minutes > 59 || seconds > 59 {
        return None;
    }
    Some(sign * (hours * 3_600 + minutes * 60 + seconds))
}

fn time_zone(identifier: &str) -> Result<TimeZone, String> {
    if let Some(seconds) = fixed_offset(identifier) {
        return Offset::from_seconds(seconds)
            .map(TimeZone::fixed)
            .map_err(|error| error.to_string());
    }
    TimeZone::get(identifier).map_err(|error| error.to_string())
}

impl TimeZoneProvider for JiffTimeZoneProvider {
    fn offset_nanoseconds(
        &self,
        identifier: &str,
        epoch_nanoseconds: i128,
    ) -> Result<i128, String> {
        const SAFE_TZDB_LIMIT_NS: i128 = 253_000_000_000_000_000_000;
        if !(-SAFE_TZDB_LIMIT_NS..=SAFE_TZDB_LIMIT_NS).contains(&epoch_nanoseconds) {
            return Err("instant is outside time-zone provider range".to_string());
        }
        let timestamp =
            Timestamp::from_nanosecond(epoch_nanoseconds).map_err(|error| error.to_string())?;
        let zone = time_zone(identifier)?;
        let offset = std::panic::catch_unwind(|| zone.to_offset(timestamp))
            .map_err(|_| "instant is outside time-zone provider range".to_string())?;
        Ok(offset.seconds() as i128 * 1_000_000_000)
    }

    fn local_to_epoch_nanoseconds(
        &self,
        identifier: &str,
        local: LocalDateTime,
        disambiguation: TimeZoneDisambiguation,
    ) -> Result<i128, String> {
        let year = i16::try_from(local.year)
            .map_err(|_| "local year is outside time-zone provider range".to_string())?;
        let datetime = DateTime::new(
            year,
            local.month as i8,
            local.day as i8,
            local.hour as i8,
            local.minute as i8,
            local.second as i8,
            local.nanosecond as i32,
        )
        .map_err(|error| error.to_string())?;
        let ambiguous = time_zone(identifier)?.to_ambiguous_timestamp(datetime);
        if disambiguation == TimeZoneDisambiguation::Reject && ambiguous.is_ambiguous() {
            return Err("ambiguous or nonexistent local time".to_string());
        }
        let timestamp = match disambiguation {
            TimeZoneDisambiguation::Compatible | TimeZoneDisambiguation::Reject => {
                ambiguous.compatible()
            }
            TimeZoneDisambiguation::Earlier => ambiguous.earlier(),
            TimeZoneDisambiguation::Later => ambiguous.later(),
        }
        .map_err(|error| error.to_string())?;
        Ok(timestamp.as_nanosecond())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_iana_offsets_at_an_instant() {
        let provider = JiffTimeZoneProvider;
        let winter = Timestamp::from_second(1_704_067_200).unwrap();
        let summer = Timestamp::from_second(1_719_792_000).unwrap();
        assert_eq!(
            provider
                .offset_nanoseconds("America/New_York", winter.as_nanosecond())
                .unwrap(),
            -5 * 3_600_000_000_000
        );
        assert_eq!(
            provider
                .offset_nanoseconds("America/New_York", summer.as_nanosecond())
                .unwrap(),
            -4 * 3_600_000_000_000
        );
    }

    #[test]
    fn distinguishes_fold_disambiguation() {
        let provider = JiffTimeZoneProvider;
        let local = LocalDateTime {
            year: 2024,
            month: 11,
            day: 3,
            hour: 1,
            minute: 30,
            second: 0,
            nanosecond: 0,
        };
        let earlier = provider
            .local_to_epoch_nanoseconds("America/New_York", local, TimeZoneDisambiguation::Earlier)
            .unwrap();
        let later = provider
            .local_to_epoch_nanoseconds("America/New_York", local, TimeZoneDisambiguation::Later)
            .unwrap();
        assert_eq!(later - earlier, 3_600_000_000_000);
        assert!(
            provider
                .local_to_epoch_nanoseconds(
                    "America/New_York",
                    local,
                    TimeZoneDisambiguation::Reject,
                )
                .is_err()
        );
    }
}
