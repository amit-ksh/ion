//! Owner calendar calculations. Rendering and clients use only a returned preview.
//! The bundled IANA database is pinned by Cargo.lock; no system zone or network lookup.
use anyhow::{Context, Result, ensure};
use jiff::{
    Timestamp, ToSpan,
    civil::Date,
    tz::{AmbiguousOffset, TimeZone, TimeZoneDatabase},
};
use std::{collections::HashSet, sync::OnceLock};
use waku_protocol::control::*;

const MAX_SEARCH_DAYS: i64 = 400;
/// Allows ordinary scheduler latency, never an unbounded catch-up queue.
pub(crate) const MISSED_GRACE_SECONDS: u64 = 60;

struct Calendar {
    rule: CalendarSchedule,
    zone: TimeZone,
}
enum Rule {
    Interval(u64),
    Calendar(Calendar),
}
impl Rule {
    fn new(request: &RoutineScheduleRequest) -> Result<Self> {
        ensure!(
            (1..=525600).contains(&request.interval_minutes),
            "Use an interval of 1–525600 minutes"
        );
        let Some(rule) = &request.calendar else {
            return Ok(Self::Interval(u64::from(request.interval_minutes) * 60));
        };
        ensure!(
            rule.hour < 24 && rule.minute < 60,
            "Use a calendar time from 00:00 to 23:59"
        );
        ensure!(
            !rule.time_zone.is_empty()
                && rule.time_zone.len() <= 128
                && rule.time_zone.trim() == rule.time_zone,
            "Enter an IANA time zone such as Asia/Kolkata or UTC"
        );
        let (days, max) = match &rule.recurrence {
            CalendarRecurrence::Daily => (&[][..], 0),
            CalendarRecurrence::Weekly { days } => {
                ensure!(
                    !days.is_empty() && days.len() <= 7,
                    "Select at least one weekday"
                );
                (days.as_slice(), 6)
            }
            CalendarRecurrence::Monthly { days } => {
                ensure!(
                    !days.is_empty() && days.len() <= 31 && !days.contains(&0),
                    "Choose calendar days from 1 through 31"
                );
                (days.as_slice(), 31)
            }
        };
        ensure!(
            days.iter().all(|day| *day <= max)
                && days.iter().collect::<HashSet<_>>().len() == days.len(),
            "Calendar days must be valid and unique"
        );
        static DATABASE: OnceLock<TimeZoneDatabase> = OnceLock::new();
        let zone = if rule.time_zone == "UTC" {
            TimeZone::UTC
        } else {
            DATABASE
                .get_or_init(TimeZoneDatabase::bundled)
                .get(&rule.time_zone)
                .context("Unknown IANA time zone; use a name such as Asia/Kolkata")?
        };
        Ok(Self::Calendar(Calendar {
            rule: rule.clone(),
            zone,
        }))
    }
    fn next(&self, after: u64) -> Result<u64> {
        match self {
            Self::Interval(seconds) => after.checked_add(*seconds).context("Schedule overflow"),
            Self::Calendar(calendar) => {
                let date = timestamp(after)?.to_zoned(calendar.zone.clone()).date();
                for offset in 0..MAX_SEARCH_DAYS {
                    if let Some(at) = calendar.on(date.checked_add(offset.days())?)? {
                        if at > after {
                            return Ok(at);
                        }
                    }
                }
                anyhow::bail!("No calendar firing was found in the next 400 days");
            }
        }
    }
    fn latest(&self, first: u64, now: u64) -> Result<u64> {
        ensure!(first > 0 && first <= now, "Routine firing is not due");
        match self {
            Self::Interval(seconds) => Ok(first + (now - first) / seconds * seconds),
            Self::Calendar(calendar) => {
                let date = timestamp(now)?.to_zoned(calendar.zone.clone()).date();
                for offset in 0..MAX_SEARCH_DAYS {
                    if let Some(at) = calendar.on(date.checked_sub(offset.days())?)? {
                        if at <= now && at >= first {
                            return Ok(at);
                        }
                    }
                }
                anyhow::bail!("Stored calendar firing no longer matches the schedule");
            }
        }
    }
}
impl Calendar {
    fn on(&self, date: Date) -> Result<Option<u64>> {
        let matches = match &self.rule.recurrence {
            CalendarRecurrence::Daily => true,
            CalendarRecurrence::Weekly { days } => {
                days.contains(&(date.weekday().to_monday_zero_offset() as u8))
            }
            CalendarRecurrence::Monthly { days } => days.contains(&(date.day() as u8)),
        };
        if !matches {
            return Ok(None);
        }
        let civil = date.at(self.rule.hour as i8, self.rule.minute as i8, 0, 0);
        let ambiguous = self.zone.to_ambiguous_timestamp(civil);
        // Skip a nonexistent clock time; only the first occurrence of a repeated
        // clock time is eligible. A stable UTC slot makes replay exactly once.
        if matches!(ambiguous.offset(), AmbiguousOffset::Gap { .. }) {
            return Ok(None);
        }
        let seconds = ambiguous.earlier()?.as_second();
        Ok(u64::try_from(seconds).ok())
    }
}
fn timestamp(at: u64) -> Result<Timestamp> {
    Ok(Timestamp::from_second(
        i64::try_from(at).context("Schedule time exceeds its range")?,
    )?)
}
pub fn validate(request: &RoutineScheduleRequest) -> Result<()> {
    Rule::new(request).map(|_| ())
}
pub(crate) fn next(request: &RoutineScheduleRequest, after: u64) -> Result<u64> {
    Rule::new(request)?.next(after)
}
pub(crate) fn due(request: &RoutineScheduleRequest, first: u64, now: u64) -> Result<(u64, u64)> {
    let rule = Rule::new(request)?;
    let slot = rule.latest(first, now)?;
    let next = rule.next(slot)?;
    ensure!(next > now, "Next calendar firing must be in the future");
    Ok((slot, next))
}
pub fn preview(request: &RoutineScheduleRequest, now: u64) -> Result<RoutineSchedulePreview> {
    let rule = Rule::new(request)?;
    let zone = match &rule {
        Rule::Interval(_) => TimeZone::UTC,
        Rule::Calendar(c) => c.zone.clone(),
    };
    let time_zone = request
        .calendar
        .as_ref()
        .map(|c| c.time_zone.clone())
        .unwrap_or_else(|| "UTC".into());
    let mut after = now;
    let mut fires = Vec::with_capacity(3);
    for _ in 0..3 {
        after = rule.next(after)?;
        fires.push(ScheduledFire {
            at: after,
            display: timestamp(after)?
                .to_zoned(zone.clone())
                .strftime("%a, %b %d %Y · %H:%M %:z")
                .to_string(),
        });
    }
    Ok(RoutineSchedulePreview {
        observed_at: now,
        time_zone,
        fires,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn at(s: &str) -> u64 {
        s.parse::<Timestamp>().unwrap().as_second() as u64
    }
    fn calendar(
        zone: &str,
        hour: u8,
        minute: u8,
        recurrence: CalendarRecurrence,
    ) -> RoutineScheduleRequest {
        RoutineScheduleRequest {
            interval_minutes: 60,
            calendar: Some(CalendarSchedule {
                time_zone: zone.into(),
                hour,
                minute,
                recurrence,
            }),
        }
    }
    #[test]
    fn calendar_schedule_iana_offsets_month_lengths_and_exact_preview() {
        let daily = calendar("Asia/Kolkata", 9, 0, CalendarRecurrence::Daily);
        let preview = preview(&daily, at("2026-10-08T02:00:00Z")).unwrap();
        assert_eq!(
            preview.fires.iter().map(|f| f.at).collect::<Vec<_>>(),
            vec![
                at("2026-10-08T03:30:00Z"),
                at("2026-10-09T03:30:00Z"),
                at("2026-10-10T03:30:00Z")
            ]
        );
        assert!(preview.fires[0].display.contains("09:00 +05:30"));
        assert_eq!(
            next(&daily, preview.fires[0].at).unwrap(),
            preview.fires[1].at
        );
        let monthly = calendar("UTC", 9, 0, CalendarRecurrence::Monthly { days: vec![31] });
        assert_eq!(
            next(&monthly, at("2024-01-31T09:00:00Z")).unwrap(),
            at("2024-03-31T09:00:00Z")
        );
        let leap = calendar("UTC", 9, 0, CalendarRecurrence::Monthly { days: vec![29] });
        assert_eq!(
            next(&leap, at("2024-02-01T00:00:00Z")).unwrap(),
            at("2024-02-29T09:00:00Z")
        );
        assert_eq!(
            next(&leap, at("2025-02-01T00:00:00Z")).unwrap(),
            at("2025-03-29T09:00:00Z")
        );
        let weekly = calendar("UTC", 9, 0, CalendarRecurrence::Weekly { days: vec![0, 4] });
        assert_eq!(
            next(&weekly, at("2026-10-08T00:00:00Z")).unwrap(),
            at("2026-10-09T09:00:00Z")
        );
        let overdue = due(
            &daily,
            at("2000-01-01T03:30:00Z"),
            at("2026-10-08T12:00:00Z"),
        )
        .unwrap();
        assert_eq!(
            overdue,
            (at("2026-10-08T03:30:00Z"), at("2026-10-09T03:30:00Z"))
        );
    }
    #[test]
    fn calendar_schedule_dst_gaps_skip_and_folds_fire_once() {
        let gap = calendar("America/New_York", 2, 30, CalendarRecurrence::Daily);
        assert_eq!(
            next(&gap, at("2024-03-09T08:00:00Z")).unwrap(),
            at("2024-03-11T06:30:00Z")
        );
        let fold = calendar("America/New_York", 1, 30, CalendarRecurrence::Daily);
        let first = next(&fold, at("2024-11-03T00:00:00Z")).unwrap();
        assert_eq!(first, at("2024-11-03T05:30:00Z"));
        assert_eq!(next(&fold, first).unwrap(), at("2024-11-04T06:30:00Z"));
        assert_eq!(
            due(&fold, first, at("2024-11-03T06:30:00Z")).unwrap().0,
            first
        );
    }
    #[test]
    fn calendar_schedule_rejects_unknown_zone_invalid_days_time_and_overflow() {
        for recurrence in [
            CalendarRecurrence::Weekly { days: vec![] },
            CalendarRecurrence::Weekly { days: vec![7] },
            CalendarRecurrence::Weekly { days: vec![0, 0] },
            CalendarRecurrence::Monthly { days: vec![0] },
            CalendarRecurrence::Monthly { days: vec![32] },
        ] {
            assert!(validate(&calendar("UTC", 9, 0, recurrence)).is_err());
        }
        assert!(validate(&calendar("not/a/zone", 9, 0, CalendarRecurrence::Daily)).is_err());
        assert!(validate(&calendar("UTC", 24, 0, CalendarRecurrence::Daily)).is_err());
        assert!(validate(&calendar("UTC", 9, 60, CalendarRecurrence::Daily)).is_err());
        assert!(next(&calendar("UTC", 9, 0, CalendarRecurrence::Daily), u64::MAX).is_err());
        let interval = RoutineScheduleRequest {
            interval_minutes: 1,
            calendar: None,
        };
        assert_eq!(due(&interval, 100, 260).unwrap(), (220, 280));
        assert!(next(&interval, u64::MAX).is_err());
    }
}
