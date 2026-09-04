//! Daily schedule key normalization (`at` / daily cron → `HHMM`).

use crate::config::types::AutoUpdateConfig;
use crate::error::{Error, Result};

/// Stable daily schedule identity, e.g. `0730` for 07:30 local time.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScheduleKey {
    key: String,
    hour: u32,
    minute: u32,
}

impl ScheduleKey {
    pub fn as_str(&self) -> &str {
        &self.key
    }

    pub fn hour_minute(&self) -> (u32, u32) {
        (self.hour, self.minute)
    }

    pub fn from_hour_minute(hour: u32, minute: u32) -> Result<Self> {
        if hour > 23 {
            return Err(Error::ScheduleError(format!("hour out of range: {hour}")));
        }
        if minute > 59 {
            return Err(Error::ScheduleError(format!(
                "minute out of range: {minute}"
            )));
        }
        Ok(Self {
            key: format!("{hour:02}{minute:02}"),
            hour,
            minute,
        })
    }

    /// Parse `auto_update` into a daily key. Errors are human-readable.
    pub fn parse_auto_update(cfg: &AutoUpdateConfig) -> Result<Self> {
        match (&cfg.at, &cfg.cron) {
            (Some(_), Some(_)) => Err(Error::ScheduleError(
                "auto_update: set only one of `at` or `cron`, not both".into(),
            )),
            (None, None) => Err(Error::ScheduleError(
                "auto_update: set `at` or `cron`".into(),
            )),
            (Some(at), None) => parse_at(at),
            (None, Some(cron)) => parse_daily_cron(cron),
        }
    }
}

impl std::fmt::Display for ScheduleKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:02}:{:02}", self.hour, self.minute)
    }
}

fn parse_at(raw: &str) -> Result<ScheduleKey> {
    let s = raw.trim();
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 2 {
        return Err(Error::ScheduleError(format!(
            "auto_update.at: expected HH:MM, got '{raw}'"
        )));
    }
    let hour: u32 = parts[0]
        .parse()
        .map_err(|_| Error::ScheduleError(format!("auto_update.at: invalid hour in '{raw}'")))?;
    let minute: u32 = parts[1]
        .parse()
        .map_err(|_| Error::ScheduleError(format!("auto_update.at: invalid minute in '{raw}'")))?;
    ScheduleKey::from_hour_minute(hour, minute).map_err(|e| match e {
        Error::ScheduleError(msg) => Error::ScheduleError(format!("auto_update.at: {msg}")),
        other => other,
    })
}

/// Accept only daily 5-field cron: `M H * * *` (minute hour DOM mon DOW).
fn parse_daily_cron(raw: &str) -> Result<ScheduleKey> {
    let s = raw.trim();
    let fields: Vec<&str> = s.split_whitespace().collect();
    if fields.len() != 5 {
        return Err(Error::ScheduleError(format!(
            "auto_update.cron: expected 5 fields, got {} in '{raw}'",
            fields.len()
        )));
    }
    let minute = parse_cron_number(fields[0], "minute")?;
    let hour = parse_cron_number(fields[1], "hour")?;
    if fields[2] != "*" || fields[3] != "*" || fields[4] != "*" {
        return Err(Error::ScheduleError(format!(
            "auto_update.cron: only daily schedules are supported in v1 (use `M H * * *` or `at`); got '{raw}'"
        )));
    }
    ScheduleKey::from_hour_minute(hour, minute).map_err(|e| match e {
        Error::ScheduleError(msg) => Error::ScheduleError(format!("auto_update.cron: {msg}")),
        other => other,
    })
}

fn parse_cron_number(field: &str, name: &str) -> Result<u32> {
    if field.contains(['*', '/', ',', '-', 'L', 'W', '#']) {
        return Err(Error::ScheduleError(format!(
            "auto_update.cron: {name} must be a single number for daily v1 schedules, got '{field}'"
        )));
    }
    field
        .parse()
        .map_err(|_| Error::ScheduleError(format!("auto_update.cron: invalid {name} '{field}'")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn at_0730_normalizes() {
        let cfg = AutoUpdateConfig {
            at: Some("07:30".into()),
            cron: None,
        };
        let key = ScheduleKey::parse_auto_update(&cfg).unwrap();
        assert_eq!(key.as_str(), "0730");
        assert_eq!(key.hour_minute(), (7, 30));
        assert_eq!(key.to_string(), "07:30");
    }

    #[test]
    fn daily_cron_matches_at() {
        let at = AutoUpdateConfig {
            at: Some("7:30".into()),
            cron: None,
        };
        let cron = AutoUpdateConfig {
            at: None,
            cron: Some("30 7 * * *".into()),
        };
        assert_eq!(
            ScheduleKey::parse_auto_update(&at).unwrap(),
            ScheduleKey::parse_auto_update(&cron).unwrap()
        );
    }

    #[test]
    fn non_daily_cron_rejected() {
        let cfg = AutoUpdateConfig {
            at: None,
            cron: Some("0 7 * * 1".into()),
        };
        let err = ScheduleKey::parse_auto_update(&cfg)
            .unwrap_err()
            .to_string();
        assert!(err.contains("daily"), "{err}");
    }

    #[test]
    fn both_at_and_cron_rejected() {
        let cfg = AutoUpdateConfig {
            at: Some("07:30".into()),
            cron: Some("30 7 * * *".into()),
        };
        let err = ScheduleKey::parse_auto_update(&cfg)
            .unwrap_err()
            .to_string();
        assert!(err.contains("only one"), "{err}");
    }

    #[test]
    fn invalid_at_rejected() {
        let cfg = AutoUpdateConfig {
            at: Some("25:00".into()),
            cron: None,
        };
        assert!(ScheduleKey::parse_auto_update(&cfg).is_err());
    }

    #[test]
    fn empty_auto_update_rejected() {
        let cfg = AutoUpdateConfig::default();
        assert!(ScheduleKey::parse_auto_update(&cfg).is_err());
    }
}
