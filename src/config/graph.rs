//! The task dependency graph.
//!
//! `depends_on` edges are consumed for: transitive closure (selection /
//! `--with-deps`), topological order within a concrete run set, dependency
//! *layers* for parallel execution, shared-dep warnings
//! ([`TaskGraph::dependents_outside`]), and validation (missing edges /
//! cycles). [`TaskGraph`] is the single home for all of it.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet, VecDeque};

use indexmap::IndexMap;

use super::types::TaskConfig;
use crate::error::{Error, Result};

/// A view over tasks and their `depends_on` edges. Borrows the task map; cheap
/// to construct.
pub struct TaskGraph<'a> {
    tasks: &'a IndexMap<String, TaskConfig>,
}

impl<'a> TaskGraph<'a> {
    pub fn new(tasks: &'a IndexMap<String, TaskConfig>) -> Self {
        Self { tasks }
    }

    /// Every `(task, missing_dependency)` pair where a task depends on a name
    /// that isn't defined. Used by validation to report each broken edge.
    pub fn missing_dependencies(&self) -> Vec<(String, String)> {
        let mut missing = Vec::new();
        for (name, task) in self.tasks {
            for dep in &task.depends_on {
                if !self.tasks.contains_key(dep) {
                    missing.push((name.clone(), dep.clone()));
                }
            }
        }
        missing
    }

    /// Find one dependency cycle, if any, returned as the path of task names
    /// forming it (e.g. `["a", "b", "a"]`). Returns `None` when the graph is
    /// acyclic. Only the first cycle encountered is reported.
    pub fn find_cycle(&self) -> Option<Vec<String>> {
        #[derive(PartialEq, Clone, Copy)]
        enum Color {
            White,
            Gray,
            Black,
        }

        let mut colors: HashMap<&str, Color> = self
            .tasks
            .keys()
            .map(|k| (k.as_str(), Color::White))
            .collect();

        fn dfs<'b>(
            node: &'b str,
            tasks: &'b IndexMap<String, TaskConfig>,
            colors: &mut HashMap<&'b str, Color>,
            path: &mut Vec<&'b str>,
        ) -> Option<Vec<String>> {
            colors.insert(node, Color::Gray);
            path.push(node);

            if let Some(task) = tasks.get(node) {
                for dep in &task.depends_on {
                    match colors.get(dep.as_str()).copied() {
                        Some(Color::Gray) => {
                            // Back edge — reconstruct the cycle path.
                            let start = path.iter().position(|&n| n == dep.as_str()).unwrap();
                            let mut cycle: Vec<String> =
                                path[start..].iter().map(|s| s.to_string()).collect();
                            cycle.push(dep.clone());
                            return Some(cycle);
                        }
                        Some(Color::White) | None => {
                            if let Some(found) = dfs(dep, tasks, colors, path) {
                                return Some(found);
                            }
                        }
                        Some(Color::Black) => {}
                    }
                }
            }

            path.pop();
            colors.insert(node, Color::Black);
            None
        }

        let keys: Vec<&str> = self.tasks.keys().map(|k| k.as_str()).collect();
        for &node in &keys {
            if colors.get(node).copied() == Some(Color::White) {
                let mut path = Vec::new();
                if let Some(cycle) = dfs(node, self.tasks, &mut colors, &mut path) {
                    return Some(cycle);
                }
            }
        }
        None
    }

    /// Seeds plus every task reachable via transitive `depends_on` edges.
    ///
    /// Errors with [`Error::MissingDependency`] if an edge points at an unknown
    /// task. Does not detect cycles (use [`Self::find_cycle`] / validate).
    pub fn closure(&self, seeds: &[String]) -> Result<Vec<String>> {
        let mut needed: HashSet<String> = HashSet::new();
        let mut queue: VecDeque<String> = seeds.iter().cloned().collect();
        while let Some(name) = queue.pop_front() {
            if needed.contains(&name) {
                continue;
            }
            if let Some(task) = self.tasks.get(&name) {
                needed.insert(name.clone());
                for dep in &task.depends_on {
                    if !self.tasks.contains_key(dep) {
                        return Err(Error::MissingDependency(name.clone(), dep.clone()));
                    }
                    if !needed.contains(dep) {
                        queue.push_back(dep.clone());
                    }
                }
            } else {
                // Unknown seed: keep it so the runner can report TaskNotFound.
                needed.insert(name);
            }
        }
        Ok(needed.into_iter().collect())
    }

    /// For each task in `run_set`, list config tasks *outside* the set that
    /// declare a direct `depends_on` edge to it. Empty dependent lists are
    /// omitted. Used for uninstall shared-dep warnings.
    pub fn dependents_outside(&self, run_set: &[String]) -> Vec<(String, Vec<String>)> {
        let in_set: HashSet<&str> = run_set.iter().map(String::as_str).collect();
        let mut out: Vec<(String, Vec<String>)> = Vec::new();
        for name in run_set {
            let mut dependents = Vec::new();
            for (other, task) in self.tasks {
                if in_set.contains(other.as_str()) {
                    continue;
                }
                if task.depends_on.iter().any(|d| d == name) {
                    dependents.push(other.clone());
                }
            }
            if !dependents.is_empty() {
                dependents.sort();
                out.push((name.clone(), dependents));
            }
        }
        out
    }

    /// Topologically order `requested` tasks using only edges whose both ends
    /// are in `requested`. Does **not** pull in unselected dependencies.
    ///
    /// When none of the requested tasks have a `depends_on` edge to another
    /// requested task, the input order is preserved and borrowed. Errors with
    /// [`Error::CyclicDependency`] if the requested subgraph contains a cycle.
    pub fn topo_order<'r>(&self, requested: &'r [String]) -> Result<Cow<'r, [String]>> {
        let requested_set: HashSet<&str> = requested.iter().map(String::as_str).collect();
        let has_internal_deps = requested.iter().filter_map(|n| self.tasks.get(n)).any(|t| {
            t.depends_on
                .iter()
                .any(|d| requested_set.contains(d.as_str()))
        });
        if !has_internal_deps {
            return Ok(Cow::Borrowed(requested));
        }

        let needed: HashSet<String> = requested.iter().cloned().collect();

        // In-degree map and dependents adjacency over the requested subgraph.
        let mut in_degree: HashMap<&str, usize> = HashMap::new();
        let mut dependents: HashMap<&str, Vec<&str>> = HashMap::new();
        for name in &needed {
            in_degree.entry(name.as_str()).or_insert(0);
            if let Some(task) = self.tasks.get(name) {
                for dep in &task.depends_on {
                    if needed.contains(dep) {
                        *in_degree.entry(name.as_str()).or_insert(0) += 1;
                        dependents
                            .entry(dep.as_str())
                            .or_default()
                            .push(name.as_str());
                    }
                }
            }
        }

        // Kahn's algorithm.
        let mut queue: VecDeque<&str> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(&name, _)| name)
            .collect();
        let mut sorted = Vec::new();
        while let Some(node) = queue.pop_front() {
            sorted.push(node.to_string());
            if let Some(deps) = dependents.get(node) {
                for &dep in deps {
                    if let Some(deg) = in_degree.get_mut(dep) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(dep);
                        }
                    }
                }
            }
        }

        if sorted.len() != needed.len() {
            let remaining: Vec<String> = needed
                .iter()
                .filter(|n| !sorted.contains(n))
                .cloned()
                .collect();
            return Err(Error::CyclicDependency(remaining.join(", ")));
        }

        Ok(Cow::Owned(sorted))
    }

    /// Group an already-ordered task list into dependency layers: tasks in the
    /// same layer have no dependencies on one another and can run in parallel.
    /// A task lands one layer deeper than its deepest dependency.
    pub fn layers(&self, ordered: &[String]) -> Vec<Vec<String>> {
        let mut layers: Vec<Vec<String>> = Vec::new();
        let mut task_layer: HashMap<&str, usize> = HashMap::new();

        for name in ordered {
            let layer = if let Some(task) = self.tasks.get(name) {
                task.depends_on
                    .iter()
                    .filter_map(|dep| task_layer.get(dep.as_str()))
                    .max()
                    .map(|&l| l + 1)
                    .unwrap_or(0)
            } else {
                0
            };

            task_layer.insert(name.as_str(), layer);
            while layers.len() <= layer {
                layers.push(Vec::new());
            }
            layers[layer].push(name.clone());
        }

        layers
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::*;

    fn task(deps: &[&str]) -> TaskConfig {
        TaskConfig {
            commands: vec![],
            os: Default::default(),
            parallel: false,
            only_if: Default::default(),
            skip_if: Default::default(),
            depends_on: deps.iter().map(|s| s.to_string()).collect(),
            retry: 0,
        }
    }

    fn graph_of(pairs: &[(&str, &[&str])]) -> IndexMap<String, TaskConfig> {
        let mut map = IndexMap::new();
        for (name, deps) in pairs {
            map.insert(name.to_string(), task(deps));
        }
        map
    }

    fn pos(order: &[String], name: &str) -> usize {
        order.iter().position(|n| n == name).unwrap()
    }

    #[test]
    fn test_topo_no_deps_preserves_and_borrows() {
        let tasks = graph_of(&[("a", &[]), ("b", &[]), ("c", &[])]);
        let g = TaskGraph::new(&tasks);
        let requested = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let ordered = g.topo_order(&requested).unwrap();
        assert!(matches!(ordered, Cow::Borrowed(_)));
        assert_eq!(&*ordered, &requested[..]);
    }

    #[test]
    fn test_topo_does_not_expand_unselected_deps() {
        // c -> b -> a; requesting only c stays exact.
        let tasks = graph_of(&[("a", &[]), ("b", &["a"]), ("c", &["b"])]);
        let g = TaskGraph::new(&tasks);
        let requested = ["c".to_string()];
        let ordered = g.topo_order(&requested).unwrap();
        assert_eq!(&*ordered, &["c".to_string()][..]);
    }

    #[test]
    fn test_closure_pulls_transitive() {
        let tasks = graph_of(&[("a", &[]), ("b", &["a"]), ("c", &["b"])]);
        let g = TaskGraph::new(&tasks);
        let mut closed = g.closure(&["c".to_string()]).unwrap();
        closed.sort();
        assert_eq!(
            closed,
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
    }

    #[test]
    fn test_closure_missing_dependency_errors() {
        let tasks = graph_of(&[("a", &["ghost"])]);
        let g = TaskGraph::new(&tasks);
        let err = g.closure(&["a".to_string()]).unwrap_err();
        assert!(matches!(err, Error::MissingDependency(t, d) if t == "a" && d == "ghost"));
    }

    #[test]
    fn test_topo_orders_within_selected_set() {
        // c -> b -> a; selecting c and a (skipping b) does not invent b.
        let tasks = graph_of(&[("a", &[]), ("b", &["a"]), ("c", &["b"])]);
        let g = TaskGraph::new(&tasks);
        let requested = ["c".to_string(), "a".to_string()];
        let ordered = g.topo_order(&requested).unwrap();
        assert_eq!(ordered.len(), 2);
        assert!(ordered.contains(&"a".to_string()));
        assert!(ordered.contains(&"c".to_string()));
        assert!(!ordered.iter().any(|n| n == "b"));
    }

    #[test]
    fn test_topo_orders_dependencies_first_when_selected() {
        let tasks = graph_of(&[("a", &[]), ("b", &["a"]), ("c", &["b"])]);
        let g = TaskGraph::new(&tasks);
        let requested = vec!["c".to_string(), "b".to_string(), "a".to_string()];
        let ordered = g.topo_order(&requested).unwrap();
        assert!(pos(&ordered, "a") < pos(&ordered, "b"));
        assert!(pos(&ordered, "b") < pos(&ordered, "c"));
    }

    #[test]
    fn test_topo_diamond() {
        let tasks = graph_of(&[("a", &[]), ("b", &["a"]), ("c", &["a"]), ("d", &["b", "c"])]);
        let g = TaskGraph::new(&tasks);
        let requested = vec![
            "d".to_string(),
            "b".to_string(),
            "c".to_string(),
            "a".to_string(),
        ];
        let ordered = g.topo_order(&requested).unwrap();
        assert_eq!(ordered.len(), 4);
        assert!(pos(&ordered, "a") < pos(&ordered, "b"));
        assert!(pos(&ordered, "a") < pos(&ordered, "c"));
        assert!(pos(&ordered, "b") < pos(&ordered, "d"));
        assert!(pos(&ordered, "c") < pos(&ordered, "d"));
    }

    #[test]
    fn test_topo_cycle_errors() {
        let tasks = graph_of(&[("a", &["b"]), ("b", &["a"])]);
        let g = TaskGraph::new(&tasks);
        let requested = ["a".to_string(), "b".to_string()];
        let err = g.topo_order(&requested).unwrap_err();
        assert!(matches!(err, Error::CyclicDependency(_)));
    }

    #[test]
    fn test_dependents_outside() {
        let tasks = graph_of(&[("base", &[]), ("leaf", &["base"]), ("other", &["base"])]);
        let g = TaskGraph::new(&tasks);
        let warnings = g.dependents_outside(&["base".to_string()]);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].0, "base");
        assert_eq!(warnings[0].1, vec!["leaf".to_string(), "other".to_string()]);
        assert!(g.dependents_outside(&["leaf".to_string()]).is_empty());
    }

    #[test]
    fn test_layers_groups_independent_tasks() {
        let tasks = graph_of(&[("a", &[]), ("b", &["a"]), ("c", &["a"]), ("d", &["b", "c"])]);
        let g = TaskGraph::new(&tasks);
        let requested = vec![
            "d".to_string(),
            "b".to_string(),
            "c".to_string(),
            "a".to_string(),
        ];
        let ordered = g.topo_order(&requested).unwrap();
        let layers = g.layers(&ordered);
        // a alone, then {b,c}, then d.
        assert_eq!(layers[0], vec!["a".to_string()]);
        assert_eq!(layers.len(), 3);
        let mut middle = layers[1].clone();
        middle.sort();
        assert_eq!(middle, vec!["b".to_string(), "c".to_string()]);
        assert_eq!(layers[2], vec!["d".to_string()]);
    }

    #[test]
    fn test_find_cycle_detects_and_reports_path() {
        let tasks = graph_of(&[("a", &["b"]), ("b", &["c"]), ("c", &["a"])]);
        let g = TaskGraph::new(&tasks);
        let cycle = g.find_cycle().expect("cycle exists");
        // First and last element close the loop.
        assert_eq!(cycle.first(), cycle.last());
        assert!(cycle.len() >= 4);
    }

    #[test]
    fn test_find_cycle_none_when_acyclic() {
        let tasks = graph_of(&[("a", &[]), ("b", &["a"])]);
        let g = TaskGraph::new(&tasks);
        assert!(g.find_cycle().is_none());
    }

    #[test]
    fn test_missing_dependencies_lists_all() {
        let tasks = graph_of(&[("a", &["x"]), ("b", &["y", "a"])]);
        let g = TaskGraph::new(&tasks);
        let mut missing = g.missing_dependencies();
        missing.sort();
        assert_eq!(
            missing,
            vec![
                ("a".to_string(), "x".to_string()),
                ("b".to_string(), "y".to_string())
            ]
        );
    }
}
