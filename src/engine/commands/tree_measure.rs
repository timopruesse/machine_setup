//! Measurement helpers for Command-bench tree ladder + ADR-0004 memory gate.

use std::fmt;

pub const CRITERION_DEFAULT_FILES: usize = 1_000;
pub const CRITERION_LARGE_FILES: usize = 10_000;
pub const MEMORY_HARNESS_FILES: usize = 100_000;
pub const PEAK_RSS_GATE_MIB: f64 = 256.0;
pub const PATHBUF_ESTIMATE_GATE_MIB: f64 = 64.0;

pub fn parse_bench_tree_size(raw: Option<&str>) -> Result<usize, String> {
    match raw.map(str::trim).filter(|s| !s.is_empty()) {
        None => Ok(CRITERION_DEFAULT_FILES),
        Some("1000") => Ok(CRITERION_DEFAULT_FILES),
        Some("10000") => Ok(CRITERION_LARGE_FILES),
        Some(other) => Err(format!(
            "MACHINE_SETUP_BENCH_TREE_SIZE={other:?} invalid; allowed: 1000, 10000"
        )),
    }
}

/// Estimate MiB for `n_files` install pairs (`src` + `dest` PathBuf payloads).
pub fn pathbuf_list_estimate_mib(n_files: usize, avg_path_bytes: f64) -> f64 {
    let bytes = (n_files as f64) * 2.0 * avg_path_bytes;
    bytes / (1024.0 * 1024.0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateVerdict {
    Pass,
    RecommendChunk,
}

impl fmt::Display for GateVerdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pass => write!(f, "PASS"),
            Self::RecommendChunk => write!(f, "RECOMMEND_CHUNK"),
        }
    }
}

pub fn gate_verdict(peak_rss_mib: Option<f64>, pathbuf_estimate_mib: f64) -> GateVerdict {
    let rss_hit = peak_rss_mib.is_some_and(|m| m >= PEAK_RSS_GATE_MIB);
    let path_hit = pathbuf_estimate_mib >= PATHBUF_ESTIMATE_GATE_MIB;
    if rss_hit || path_hit {
        GateVerdict::RecommendChunk
    } else {
        GateVerdict::Pass
    }
}

/// Peak RSS of this process in MiB, or `None` if unsupported.
pub fn peak_rss_mib() -> Option<f64> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        // SAFETY: getrusage with RUSAGE_SELF is well-defined.
        unsafe {
            let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
            if libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) != 0 {
                return None;
            }
            let usage = usage.assume_init();
            let raw = usage.ru_maxrss as f64;
            #[cfg(target_os = "linux")]
            {
                // Linux: KiB
                Some(raw / 1024.0)
            }
            #[cfg(target_os = "macos")]
            {
                // macOS: bytes
                Some(raw / (1024.0 * 1024.0))
            }
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{gate_verdict, parse_bench_tree_size, pathbuf_list_estimate_mib, GateVerdict};

    #[test]
    fn parse_default_and_allowed() {
        assert_eq!(parse_bench_tree_size(None).unwrap(), 1_000);
        assert_eq!(parse_bench_tree_size(Some("")).unwrap(), 1_000);
        assert_eq!(parse_bench_tree_size(Some("1000")).unwrap(), 1_000);
        assert_eq!(parse_bench_tree_size(Some("10000")).unwrap(), 10_000);
    }

    #[test]
    fn parse_rejects_unknown() {
        assert!(parse_bench_tree_size(Some("100000")).is_err());
        assert!(parse_bench_tree_size(Some("5000")).is_err());
    }

    #[test]
    fn pathbuf_estimate_scales() {
        // 100_000 files * 2 paths * 100 bytes = 20_000_000 bytes ≈ 19.07 MiB
        let mib = pathbuf_list_estimate_mib(100_000, 100.0);
        assert!((mib - 20_000_000.0 / (1024.0 * 1024.0)).abs() < 0.01);
    }

    #[test]
    fn verdict_rss_or_pathbuf() {
        assert!(matches!(gate_verdict(Some(255.0), 63.0), GateVerdict::Pass));
        assert!(matches!(
            gate_verdict(Some(256.0), 0.0),
            GateVerdict::RecommendChunk
        ));
        assert!(matches!(
            gate_verdict(None, 64.0),
            GateVerdict::RecommendChunk
        ));
        assert!(matches!(gate_verdict(None, 63.9), GateVerdict::Pass));
    }
}
