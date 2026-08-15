//! Mode-aware task selection: expand `depends_on` for install / `--with-deps`,
//! and helpers for uninstall dep prompts and shared-dep warnings.

use super::graph::TaskGraph;
use super::types::AppConfig;
use crate::engine::mode::Mode;
use crate::error::Result;

/// Expand `selected` according to mode policy.
///
/// - **Install** always returns the transitive closure.
/// - **Update** / **Uninstall** return `selected` unchanged unless `with_deps`.
pub fn expand_for_mode(
    config: &AppConfig,
    selected: &[String],
    mode: Mode,
    with_deps: bool,
) -> Result<Vec<String>> {
    let should_expand = matches!(mode, Mode::Install) || with_deps;
    if !should_expand {
        return Ok(selected.to_vec());
    }
    let graph = TaskGraph::new(&config.tasks);
    graph.closure(selected)
}

/// Transitive deps of `selected` that are not already in `selected`, sorted.
/// Candidates for the uninstall “also uninstall?” multi-select.
pub fn uninstall_dep_candidates(config: &AppConfig, selected: &[String]) -> Result<Vec<String>> {
    let graph = TaskGraph::new(&config.tasks);
    let closed = graph.closure(selected)?;
    let selected_set: std::collections::HashSet<&str> =
        selected.iter().map(String::as_str).collect();
    let mut extras: Vec<String> = closed
        .into_iter()
        .filter(|n| !selected_set.contains(n.as_str()))
        .collect();
    extras.sort();
    Ok(extras)
}

/// Union `selected` with `extras` (deduped, extras appended in given order).
pub fn apply_extra_deps(selected: Vec<String>, extras: Vec<String>) -> Vec<String> {
    let mut set: std::collections::HashSet<String> = selected.iter().cloned().collect();
    let mut out = selected;
    for name in extras {
        if set.insert(name.clone()) {
            out.push(name);
        }
    }
    out
}

/// Tasks in `run_set` that other config tasks outside the set still depend on.
pub fn shared_dep_warnings(config: &AppConfig, run_set: &[String]) -> Vec<(String, Vec<String>)> {
    TaskGraph::new(&config.tasks).dependents_outside(run_set)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::*;
    use indexmap::IndexMap;

    fn task(deps: &[&str]) -> TaskConfig {
        TaskConfig {
            commands: vec![],
            os: Default::default(),
            parallel: false,
            only_if: Default::default(),
            skip_if: Default::default(),
            depends_on: deps.iter().map(|s| s.to_string()).collect(),
            retry: 0,
            auto_update: None,
        }
    }

    fn config(pairs: &[(&str, &[&str])]) -> AppConfig {
        let mut tasks = IndexMap::new();
        for (name, deps) in pairs {
            tasks.insert(name.to_string(), task(deps));
        }
        AppConfig {
            tasks,
            temp_dir: "~/.machine_setup".to_string(),
            default_shell: Shell::Bash,
            parallel: false,
            num_threads: None,
        }
    }

    #[test]
    fn install_always_expands() {
        let cfg = config(&[("a", &[]), ("b", &["a"]), ("c", &["b"])]);
        let mut got = expand_for_mode(&cfg, &["c".to_string()], Mode::Install, false).unwrap();
        got.sort();
        assert_eq!(got, vec!["a".to_string(), "b".to_string(), "c".to_string()]);
    }

    #[test]
    fn update_exact_unless_with_deps() {
        let cfg = config(&[("a", &[]), ("b", &["a"])]);
        let exact = expand_for_mode(&cfg, &["b".to_string()], Mode::Update, false).unwrap();
        assert_eq!(exact, vec!["b".to_string()]);
        let mut with = expand_for_mode(&cfg, &["b".to_string()], Mode::Update, true).unwrap();
        with.sort();
        assert_eq!(with, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn uninstall_candidates_exclude_selected() {
        let cfg = config(&[("a", &[]), ("b", &["a"]), ("c", &["b"])]);
        let candidates = uninstall_dep_candidates(&cfg, &["c".to_string()]).unwrap();
        assert_eq!(candidates, vec!["a".to_string(), "b".to_string()]);
        let none =
            uninstall_dep_candidates(&cfg, &["a".to_string(), "b".to_string(), "c".to_string()])
                .unwrap();
        assert!(none.is_empty());
    }

    #[test]
    fn apply_extra_deps_unions() {
        let out = apply_extra_deps(
            vec!["c".to_string()],
            vec!["a".to_string(), "c".to_string()],
        );
        assert_eq!(out, vec!["c".to_string(), "a".to_string()]);
    }

    #[test]
    fn shared_warnings_list_outside_dependents() {
        let cfg = config(&[("base", &[]), ("leaf", &["base"]), ("other", &[])]);
        let w = shared_dep_warnings(&cfg, &["base".to_string()]);
        assert_eq!(w, vec![("base".to_string(), vec!["leaf".to_string()])]);
    }
}
