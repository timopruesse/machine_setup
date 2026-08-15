//! Fallback when OS timers are not supported.

use crate::error::{Error, Result};
use crate::schedule::key::ScheduleKey;
use crate::schedule::platform::{PlatformUnits, UnitSpec};

pub struct Unsupported;

impl PlatformUnits for Unsupported {
    fn label_for_key(&self, key: &ScheduleKey) -> String {
        format!("machine_setup.schedule.{}", key.as_str())
    }

    fn apply_unit(&self, _spec: &UnitSpec) -> Result<()> {
        Err(Error::ScheduleError(
            "schedule apply is only supported on macOS (launchd) and Linux (systemd user timers)"
                .into(),
        ))
    }

    fn remove_unit(&self, _label: &str) -> Result<()> {
        Err(Error::ScheduleError(
            "schedule remove is only supported on macOS and Linux".into(),
        ))
    }
}
