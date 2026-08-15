//! Daily schedule key normalization (`at` / daily cron → `HHMM`).

use crate::config::types::AutoUpdateConfig;

/// Stable daily schedule identity, e.g. `0730` for 07:30 local time.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ScheduleKey(String);

impl ScheduleKey {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn hour_minute(&self) -> (u32, u32) {
        let hour: u32 = self.0[0..2].parse().expect("ScheduleKey hour");
        let minute: u32 = self.0[2..4].parse().expect("ScheduleKey minute");
        (hour, minute)
    }

    pub fn from_hour_minute(hour: u32, minute: u32) -> Result<Self, String> {
        if hour > 23 {
            return Err(format!("hour out of range: {hour}"));
        }
        if minute > 59 {
            return Err(format!("minute out of range: {minute}"));
        }
        Ok(Self(format!("{hour:02}{minute:02}")))
    }

    /// Parse `auto_update` into a daily key. Errors are human-readable.
    pub fn parse_auto_update(cfg: &AutoUpdateConfig) -> Result<Self, String> {
        match (&cfg.at, &cfg.cron) {
            (Some(_), Some(_)) => {
                Err("auto_update: set only one of `at` or `cron`, not both".to_string())
            }
            (None, None) => Err("auto_update: set `at` or `cron`".to_string()),
            (Some(at), None) => parse_at(at),
            (None, Some(cron)) => parse_daily_cron(cron),
        }
    }
}

impl std::fmt::Display for ScheduleKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (h, m) = self.hour_minute();
        write!(f, "{h:02}:{m:02}")
    }
}

fn parse_at(raw: &str) -> Result<ScheduleKey, String> {
    let s = raw.trim();
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 2 {
        return Err(format!("auto_update.at: expected HH:MM, got '{raw}'"));
    }
    let hour: u32 = parts[0]
        .parse()
        .map_err(|_| format!("auto_update.at: invalid hour in '{raw}'"))?;
    let minute: u32 = parts[1]
        .parse()
        .map_err(|_| format!("auto_update.at: invalid minute in '{raw}'"))?;
    ScheduleKey::from_hour_minute(hour, minute).map_err(|e| format!("auto_update.at: {e}"))
}

/// Accept only daily 5-field cron: `M H * * *` (minute hour DOM mon DOW).
fn parse_daily_cron(raw: &str) -> Result<ScheduleKey, String> {
    let s = raw.trim();
    let fields: Vec<&str> = s.split_whitespace().collect();
    if fields.len() != 5 {
        return Err(format!(
            "auto_update.cron: expected 5 fields, got {} in '{raw}'",
            fields.len()
        ));
    }
    let minute = parse_cron_number(fields[0], "minute")?;
    let hour = parse_cron_number(fields[1], "hour")?;
    if fields[2] != "*" || fields[3] != "*" || fields[4] != "*" {
        return Err(format!(
            "auto_update.cron: only daily schedules are supported in v1 (use `M H * * *` or `at`); got '{raw}'"
        ));
    }
    ScheduleKey::from_hour_minute(hour, minute).map_err(|e| format!("auto_update.cron: {e}"))
}

fn parse_cron_number(field: &str, name: &str) -> Result<u32, String> {
    if field.contains(['*', '/', ',', '-', 'L', 'W', '#']) {
        return Err(format!(
            "auto_update.cron: {name} must be a single number for daily v1 schedules, got '{field}'"
        ));
    }
    field
        .parse()
        .map_err(|_| format!("auto_update.cron: invalid {name} '{field}'"))
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
        let err = ScheduleKey::parse_auto_update(&cfg).unwrap_err();
        assert!(err.contains("daily"), "{err}");
    }

    #[test]
    fn both_at_and_cron_rejected() {
        let cfg = AutoUpdateConfig {
            at: Some("07:30".into()),
            cron: Some("30 7 * * *".into()),
        };
        assert!(ScheduleKey::parse_auto_update(&cfg)
            .unwrap_err()
            .contains("only one"));
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
