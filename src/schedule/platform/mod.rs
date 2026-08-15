//! Platform OS timer unit installers.

use std::path::PathBuf;

use crate::error::Result;
use crate::schedule::key::ScheduleKey;

pub mod launchd;
pub mod systemd;
pub mod unsupported;

/// Spec for one bundled daily schedule unit.
#[derive(Debug, Clone)]
pub struct UnitSpec {
    pub key: ScheduleKey,
    pub binary: PathBuf,
    pub config_path: PathBuf,
}

impl UnitSpec {
    pub fn hour_minute(&self) -> (u32, u32) {
        self.key.hour_minute()
    }
}

pub trait PlatformUnits {
    fn label_for_key(&self, key: &ScheduleKey) -> String;
    fn apply_unit(&self, spec: &UnitSpec) -> Result<()>;
    fn remove_unit(&self, label: &str) -> Result<()>;
}

/// Select the native backend for this host.
pub fn current() -> Box<dyn PlatformUnits> {
    #[cfg(target_os = "macos")]
    {
        Box::new(launchd::Launchd)
    }
    #[cfg(target_os = "linux")]
    {
        Box::new(systemd::SystemdUser)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        Box::new(unsupported::Unsupported)
    }
}

/// Launchd label / systemd unit stem helpers shared by status text.
pub fn launchd_label(key: &ScheduleKey) -> String {
    format!("com.machine_setup.schedule.{}", key.as_str())
}

pub fn systemd_unit_stem(key: &ScheduleKey) -> String {
    format!("machine_setup-schedule-{}", key.as_str())
}
