use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Os {
    Linux,
    Macos,
    Windows,
    Ios,
    Freebsd,
    Dragonfly,
    Netbsd,
    Openbsd,
    Solaris,
    Android,
}

impl Os {
    pub fn current() -> Option<Self> {
        match std::env::consts::OS {
            "linux" => Some(Os::Linux),
            "macos" => Some(Os::Macos),
            "windows" => Some(Os::Windows),
            "ios" => Some(Os::Ios),
            "freebsd" => Some(Os::Freebsd),
            "dragonfly" => Some(Os::Dragonfly),
            "netbsd" => Some(Os::Netbsd),
            "openbsd" => Some(Os::Openbsd),
            "solaris" => Some(Os::Solaris),
            "android" => Some(Os::Android),
            _ => None,
        }
    }
}

impl std::fmt::Display for Os {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Os::Linux => write!(f, "linux"),
            Os::Macos => write!(f, "macos"),
            Os::Windows => write!(f, "windows"),
            Os::Ios => write!(f, "ios"),
            Os::Freebsd => write!(f, "freebsd"),
            Os::Dragonfly => write!(f, "dragonfly"),
            Os::Netbsd => write!(f, "netbsd"),
            Os::Openbsd => write!(f, "openbsd"),
            Os::Solaris => write!(f, "solaris"),
            Os::Android => write!(f, "android"),
        }
    }
}

/// Represents OS filtering: either a single OS or multiple.
/// When empty/None, the task runs on all OSes.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OsFilter {
    #[default]
    All,
    Single(Os),
    Multiple(Vec<Os>),
}

impl OsFilter {
    pub fn matches_current(&self) -> bool {
        match self {
            OsFilter::All => true,
            OsFilter::Single(os) => Os::current().as_ref() == Some(os),
            OsFilter::Multiple(oses) => {
                Os::current().as_ref().is_some_and(|current| oses.contains(current))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_os_current_is_some() {
        assert!(Os::current().is_some());
    }

    #[test]
    fn test_os_filter_all_matches() {
        assert!(OsFilter::All.matches_current());
    }

    #[test]
    fn test_os_filter_deserialize_single() {
        let filter: OsFilter = serde_json::from_str(r#""linux""#).unwrap();
        assert!(matches!(filter, OsFilter::Single(Os::Linux)));
    }

    #[test]
    fn test_os_filter_deserialize_multiple() {
        let filter: OsFilter = serde_json::from_str(r#"["linux", "macos"]"#).unwrap();
        assert!(matches!(filter, OsFilter::Multiple(_)));
    }
}
