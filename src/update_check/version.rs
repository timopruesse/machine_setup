//! Semver-ish X.Y.Z compare for update checks (no semver crate).

/// Strip a leading `v` / `V` and parse `major.minor.patch` (extra pre-release suffix ignored after `-`).
pub fn parse_triple(raw: &str) -> Option<(u64, u64, u64)> {
    let s = raw.trim().trim_start_matches(['v', 'V']);
    let core = s.split('-').next().unwrap_or(s);
    let mut parts = core.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    Some((major, minor, patch))
}

/// True when `remote` is a greater X.Y.Z than `current`.
pub fn is_newer(remote: &str, current: &str) -> bool {
    match (parse_triple(remote), parse_triple(current)) {
        (Some(r), Some(c)) => r > c,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_versions() {
        assert!(is_newer("2.7.0", "2.6.1"));
        assert!(is_newer("v2.7.0", "2.6.1"));
        assert!(!is_newer("2.6.1", "2.6.1"));
        assert!(!is_newer("2.5.0", "2.6.1"));
        assert!(!is_newer("not-a-version", "2.6.1"));
    }
}
