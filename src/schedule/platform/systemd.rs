//! Linux systemd user timer + service units.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{Error, Result};
use crate::schedule::key::ScheduleKey;
use crate::schedule::platform::{systemd_unit_stem, PlatformUnits, UnitSpec};

pub struct SystemdUser;

impl PlatformUnits for SystemdUser {
    fn label_for_key(&self, key: &ScheduleKey) -> String {
        systemd_unit_stem(key)
    }

    fn apply_unit(&self, spec: &UnitSpec) -> Result<()> {
        let stem = self.label_for_key(&spec.key);
        let dir = user_unit_dir()?;
        std::fs::create_dir_all(&dir)?;
        let (hour, minute) = spec.hour_minute();
        let service = render_service(&spec.binary, &spec.config_path, spec.key.as_str());
        let timer = render_timer(&stem, hour, minute);
        std::fs::write(dir.join(format!("{stem}.service")), service)?;
        std::fs::write(dir.join(format!("{stem}.timer")), timer)?;
        daemon_reload()?;
        enable_now(&format!("{stem}.timer"))?;
        Ok(())
    }

    fn remove_unit(&self, label: &str) -> Result<()> {
        let dir = user_unit_dir()?;
        let timer = format!("{label}.timer");
        let _ = Command::new("systemctl")
            .args(["--user", "disable", "--now", &timer])
            .output();
        let _ = std::fs::remove_file(dir.join(format!("{label}.timer")));
        let _ = std::fs::remove_file(dir.join(format!("{label}.service")));
        let _ = daemon_reload();
        Ok(())
    }
}

fn user_unit_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| Error::ScheduleError("no home directory".into()))?;
    Ok(home.join(".config/systemd/user"))
}

pub fn render_service(binary: &Path, config: &Path, key: &str) -> String {
    format!(
        r#"[Unit]
Description=machine_setup schedule run ({key})

[Service]
Type=oneshot
ExecStart={bin} schedule run --key {key} --config {cfg} --no-tui
"#,
        bin = shell_escape(&binary.display().to_string()),
        cfg = shell_escape(&config.display().to_string()),
        key = key,
    )
}

pub fn render_timer(stem: &str, hour: u32, minute: u32) -> String {
    format!(
        r#"[Unit]
Description=machine_setup schedule timer ({stem})

[Timer]
OnCalendar=*-*-* {hour:02}:{minute:02}:00
Persistent=true
Unit={stem}.service

[Install]
WantedBy=timers.target
"#
    )
}

fn shell_escape(s: &str) -> String {
    if s.chars().any(|c| c.is_whitespace()) {
        format!("\"{}\"", s.replace('"', "\\\""))
    } else {
        s.to_string()
    }
}

fn daemon_reload() -> Result<()> {
    let out = Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .output()
        .map_err(|e| Error::ScheduleError(format!("systemctl daemon-reload: {e}")))?;
    if !out.status.success() {
        return Err(Error::ScheduleError(format!(
            "systemctl daemon-reload failed: {}",
            String::from_utf8_lossy(&out.stderr)
        )));
    }
    Ok(())
}

fn enable_now(timer: &str) -> Result<()> {
    let out = Command::new("systemctl")
        .args(["--user", "enable", "--now", timer])
        .output()
        .map_err(|e| Error::ScheduleError(format!("systemctl enable: {e}")))?;
    if !out.status.success() {
        return Err(Error::ScheduleError(format!(
            "systemctl enable --now {timer} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timer_is_persistent_daily() {
        let t = render_timer("machine_setup-schedule-0730", 7, 30);
        assert!(t.contains("Persistent=true"));
        assert!(t.contains("OnCalendar=*-*-* 07:30:00"));
        assert!(t.contains("machine_setup-schedule-0730.service"));
    }

    #[test]
    fn service_invokes_schedule_run() {
        let s = render_service(
            Path::new("/usr/bin/machine_setup"),
            Path::new("/home/u/cfg.yaml"),
            "0730",
        );
        assert!(s.contains("schedule run --key 0730"));
        assert!(s.contains("--no-tui"));
    }
}
