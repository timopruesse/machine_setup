//! Fetch latest GitHub release tag (no live network in unit tests).

use std::time::Duration;

use serde::Deserialize;
use ureq::Agent;

use crate::error::{Error, Result};

const RELEASES_LATEST: &str =
    "https://api.github.com/repos/timopruesse/machine_setup/releases/latest";

#[derive(Debug, Deserialize)]
struct LatestRelease {
    tag_name: String,
}

/// Fetch latest release tag (e.g. `v2.7.0`). Timeout ~2s.
pub fn fetch_latest_tag() -> Result<String> {
    let agent: Agent = Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(2)))
        .http_status_as_error(true)
        .build()
        .into();

    let body = agent
        .get(RELEASES_LATEST)
        .header("User-Agent", "machine_setup-update-check")
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| Error::Other(format!("update check fetch failed: {e}")))?
        .into_body()
        .read_to_string()
        .map_err(|e| Error::Other(format!("update check read failed: {e}")))?;

    parse_tag_name(&body)
}

pub fn parse_tag_name(json: &str) -> Result<String> {
    let release: LatestRelease =
        serde_json::from_str(json).map_err(|e| Error::Other(format!("update check parse: {e}")))?;
    if release.tag_name.trim().is_empty() {
        return Err(Error::Other("empty tag_name".into()));
    }
    Ok(release.tag_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_github_fixture() {
        let json = r#"{"tag_name":"v2.7.0","name":"2.7.0"}"#;
        assert_eq!(parse_tag_name(json).unwrap(), "v2.7.0");
    }
}
