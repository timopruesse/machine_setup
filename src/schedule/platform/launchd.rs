//! macOS launchd user agent units.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{Error, Result};
use crate::schedule::key::ScheduleKey;
use crate::schedule::platform::{launchd_label, PlatformUnits, UnitSpec};

pub struct Launchd;

impl PlatformUnits for Launchd {
    fn label_for_key(&self, key: &ScheduleKey) -> String {
        launchd_label(key)
    }

    fn apply_unit(&self, spec: &UnitSpec) -> Result<()> {
        let label = self.label_for_key(&spec.key);
        let plist_path = plist_path(&label)?;
        let (hour, minute) = spec.hour_minute();
        let xml = render_plist(
            &label,
            &spec.binary,
            &spec.config_path,
            spec.key.as_str(),
            hour,
            minute,
        );
        if let Some(parent) = plist_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // bootout first if present (ignore errors)
        let _ = bootout(&label, &plist_path);
        std::fs::write(&plist_path, xml)?;
        bootstrap(&label, &plist_path)?;
        Ok(())
    }

    fn remove_unit(&self, label: &str) -> Result<()> {
        let plist_path = plist_path(label)?;
        let _ = bootout(label, &plist_path);
        if plist_path.exists() {
            std::fs::remove_file(&plist_path)?;
        }
        Ok(())
    }
}

fn agents_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| Error::ScheduleError("no home directory".into()))?;
    Ok(home.join("Library/LaunchAgents"))
}

fn plist_path(label: &str) -> Result<PathBuf> {
    Ok(agents_dir()?.join(format!("{label}.plist")))
}

pub fn render_plist(
    label: &str,
    binary: &Path,
    config: &Path,
    key: &str,
    hour: u32,
    minute: u32,
) -> String {
    let bin = xml_escape(&binary.display().to_string());
    let cfg = xml_escape(&config.display().to_string());
    let label_esc = xml_escape(label);
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{label_esc}</string>
  <key>ProgramArguments</key>
  <array>
    <string>{bin}</string>
    <string>schedule</string>
    <string>run</string>
    <string>--key</string>
    <string>{key}</string>
    <string>--config</string>
    <string>{cfg}</string>
    <string>--no-tui</string>
  </array>
  <key>StartCalendarInterval</key>
  <dict>
    <key>Hour</key>
    <integer>{hour}</integer>
    <key>Minute</key>
    <integer>{minute}</integer>
  </dict>
  <key>RunAtLoad</key>
  <false/>
</dict>
</plist>
"#
    )
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn uid() -> Result<u32> {
    let out = Command::new("id")
        .arg("-u")
        .output()
        .map_err(|e| Error::ScheduleError(format!("id -u failed: {e}")))?;
    if !out.status.success() {
        return Err(Error::ScheduleError("id -u failed".into()));
    }
    let s = String::from_utf8_lossy(&out.stdout);
    s.trim()
        .parse()
        .map_err(|_| Error::ScheduleError(format!("invalid uid: {s}")))
}

fn bootstrap(label: &str, plist: &Path) -> Result<()> {
    let domain = format!("gui/{}", uid()?);
    let out = Command::new("launchctl")
        .args(["bootstrap", &domain, plist.to_str().unwrap_or_default()])
        .output()
        .map_err(|e| Error::ScheduleError(format!("launchctl bootstrap failed: {e}")))?;
    if !out.status.success() {
        // Fallback older API
        let out2 = Command::new("launchctl")
            .args(["load", "-w", plist.to_str().unwrap_or_default()])
            .output()
            .map_err(|e| Error::ScheduleError(format!("launchctl load failed: {e}")))?;
        if !out2.status.success() {
            return Err(Error::ScheduleError(format!(
                "launchctl bootstrap/load failed for {label}: {}",
                String::from_utf8_lossy(&out.stderr)
            )));
        }
    }
    Ok(())
}

fn bootout(label: &str, plist: &Path) -> Result<()> {
    let domain = format!("gui/{}", uid()?);
    let _ = Command::new("launchctl")
        .args(["bootout", &format!("{domain}/{label}")])
        .output();
    let _ = Command::new("launchctl")
        .args(["unload", plist.to_str().unwrap_or_default()])
        .output();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn plist_contains_calendar_and_args() {
        let xml = render_plist(
            "com.machine_setup.schedule.0730",
            Path::new("/usr/local/bin/machine_setup"),
            Path::new("/Users/me/machine_setup.yaml"),
            "0730",
            7,
            30,
        );
        assert!(xml.contains("<integer>7</integer>"));
        assert!(xml.contains("<integer>30</integer>"));
        assert!(xml.contains("schedule"));
        assert!(xml.contains("--key"));
        assert!(xml.contains("0730"));
        assert!(xml.contains("/usr/local/bin/machine_setup"));
        let _ = PathBuf::from("/tmp");
    }
}
