use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

use crate::cli::Command;
use crate::config::history::History;
use crate::config::types::{AppConfig, TaskConfig};
use crate::error::{Error, Result};
use crate::utils::path::expand_path;

use super::commands::{create_executor, CommandExecutor};
use super::context::CommandContext;
use super::event::TaskEvent;

pub struct TaskRunner {
    config: AppConfig,
    mode: Command,
    event_tx: mpsc::UnboundedSender<TaskEvent>,
    config_dir: PathBuf,
    depth: usize,
}

impl TaskRunner {
    pub fn new(
        config: AppConfig,
        mode: Command,
        event_tx: mpsc::UnboundedSender<TaskEvent>,
    ) -> Self {
        Self {
            config,
            mode,
            event_tx,
            config_dir: std::env::current_dir().unwrap_or_default(),
            depth: 0,
        }
    }

    pub fn with_config_dir(mut self, dir: PathBuf) -> Self {
        self.config_dir = dir;
        self
    }

    pub fn with_depth(mut self, depth: usize) -> Self {
        self.depth = depth;
        self
    }

    /// Run all tasks (respecting parallel config).
    pub async fn run_all(&self, force: bool) -> Result<()> {
        let task_names: Vec<String> = self.config.tasks.keys().cloned().collect();
        self.run_tasks(&task_names, force).await
    }

    /// Run a single task by name.
    pub async fn run_single_task(&self, task_name: &str, force: bool) -> Result<()> {
        if !self.config.tasks.contains_key(task_name) {
            return Err(Error::TaskNotFound(task_name.to_string()));
        }
        self.run_tasks(&[task_name.to_string()], force).await
    }

    /// Run specific tasks by name.
    pub async fn run_tasks(&self, task_names: &[String], force: bool) -> Result<()> {
        // Resolve dependency order via topological sort
        let ordered = self.topological_sort(task_names)?;

        let temp_dir = expand_path(&self.config.temp_dir, None);
        let mut history = History::load(&temp_dir).unwrap_or_default();

        let mut succeeded = 0usize;
        let mut failed = 0usize;
        let mut skipped = 0usize;

        if self.config.parallel {
            // Parallel execution with dependency layers
            let layers = self.dependency_layers(&ordered);

            for layer in layers {
                let mut handles = Vec::new();

                for name in &layer {
                    let task_config = &self.config.tasks[name];

                    if let Some(reason) = self.should_skip(task_config, name, force, &history) {
                        self.send(TaskEvent::TaskSkipped {
                            task_name: name.clone(),
                            reason,
                        });
                        skipped += 1;
                        continue;
                    }

                    let ctx = self.create_context(name, &temp_dir);
                    let task = task_config.clone();
                    let mode = self.mode.clone();
                    let depth = self.depth;
                    let name = name.clone();
                    handles.push(tokio::spawn(async move {
                        let result = run_task_with_retry(&name, &task, &ctx, mode, depth).await;
                        (name, result)
                    }));
                }

                for handle in handles {
                    match handle.await {
                        Ok((name, Ok(()))) => {
                            self.update_history(&mut history, &name);
                            succeeded += 1;
                        }
                        Ok((name, Err(e))) => {
                            self.send(TaskEvent::TaskFailed {
                                task_name: name,
                                error: e.to_string(),
                            });
                            failed += 1;
                        }
                        Err(_) => {
                            failed += 1;
                        }
                    }
                }
            }
        } else {
            // Sequential execution
            for name in &ordered {
                let task_config = &self.config.tasks[name];

                if let Some(reason) = self.should_skip(task_config, name, force, &history) {
                    self.send(TaskEvent::TaskSkipped {
                        task_name: name.clone(),
                        reason,
                    });
                    skipped += 1;
                    continue;
                }

                let ctx = self.create_context(name, &temp_dir);

                match run_task_with_retry(name, task_config, &ctx, self.mode.clone(), self.depth)
                    .await
                {
                    Ok(()) => {
                        self.update_history(&mut history, name);
                        succeeded += 1;
                    }
                    Err(e) => {
                        self.send(TaskEvent::TaskFailed {
                            task_name: name.clone(),
                            error: e.to_string(),
                        });
                        failed += 1;
                    }
                }
            }
        }

        // Save history
        if let Err(e) = history.save(&temp_dir) {
            tracing::warn!("Failed to save history: {e}");
        }

        self.send(TaskEvent::AllDone {
            succeeded,
            failed,
            skipped,
        });

        if failed > 0 {
            Err(Error::Other(format!("{failed} task(s) failed")))
        } else {
            Ok(())
        }
    }

    /// Check if a task should be skipped (OS filter, conditions, history).
    fn should_skip(
        &self,
        task: &TaskConfig,
        name: &str,
        force: bool,
        history: &History,
    ) -> Option<String> {
        // Check OS filter
        if !task.os.matches_current() {
            return Some("OS mismatch".to_string());
        }

        // Check only_if conditions
        for path_str in task.only_if.as_slice() {
            let path = expand_path(path_str, Some(&self.config_dir));
            if !path.exists() {
                return Some(format!("Condition not met: '{path_str}' does not exist"));
            }
        }

        // Check skip_if conditions
        for path_str in task.skip_if.as_slice() {
            let path = expand_path(path_str, Some(&self.config_dir));
            if path.exists() {
                return Some(format!("Skipped: '{path_str}' exists"));
            }
        }

        // Check history
        if self.mode == Command::Install && !force && history.is_installed(name) {
            return Some("Already installed (use --force to reinstall)".to_string());
        }

        None
    }

    /// Topological sort of tasks respecting depends_on.
    /// Returns tasks in dependency order. Includes transitive dependencies
    /// of the requested tasks.
    fn topological_sort(&self, requested: &[String]) -> Result<Vec<String>> {
        let all_tasks = &self.config.tasks;

        // If no task has dependencies, preserve original order
        let has_deps = requested
            .iter()
            .filter_map(|n| all_tasks.get(n))
            .any(|t| !t.depends_on.is_empty());
        if !has_deps {
            return Ok(requested.to_vec());
        }

        // Collect all needed tasks (requested + transitive deps)
        let mut needed: HashSet<String> = HashSet::new();
        let mut queue: VecDeque<String> = requested.iter().cloned().collect();
        while let Some(name) = queue.pop_front() {
            if needed.contains(&name) {
                continue;
            }
            if let Some(task) = all_tasks.get(&name) {
                needed.insert(name.clone());
                for dep in &task.depends_on {
                    if !all_tasks.contains_key(dep) {
                        return Err(Error::MissingDependency(name.clone(), dep.clone()));
                    }
                    if !needed.contains(dep) {
                        queue.push_back(dep.clone());
                    }
                }
            }
        }

        // Build in-degree map for needed tasks
        let mut in_degree: HashMap<&str, usize> = HashMap::new();
        let mut dependents: HashMap<&str, Vec<&str>> = HashMap::new();
        for name in &needed {
            in_degree.entry(name.as_str()).or_insert(0);
            if let Some(task) = all_tasks.get(name) {
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

        // Kahn's algorithm
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

        Ok(sorted)
    }

    /// Group tasks into dependency layers for parallel execution.
    /// Tasks in the same layer have no dependencies on each other.
    fn dependency_layers(&self, ordered: &[String]) -> Vec<Vec<String>> {
        let all_tasks = &self.config.tasks;
        let mut layers: Vec<Vec<String>> = Vec::new();
        let mut task_layer: HashMap<&str, usize> = HashMap::new();

        for name in ordered {
            let layer = if let Some(task) = all_tasks.get(name) {
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

    /// Get ordered task names (for list command / TUI display).
    #[allow(dead_code)]
    pub fn task_names(&self) -> Vec<String> {
        self.config.tasks.keys().cloned().collect()
    }

    /// Get task configs for display.
    #[allow(dead_code)]
    pub fn tasks(&self) -> &indexmap::IndexMap<String, TaskConfig> {
        &self.config.tasks
    }

    fn create_context(&self, task_name: &str, temp_dir: &Path) -> CommandContext {
        CommandContext {
            event_tx: self.event_tx.clone(),
            mode: self.mode.clone(),
            config_dir: self.config_dir.clone(),
            temp_dir: temp_dir.to_path_buf(),
            default_shell: self.config.default_shell.clone(),
            task_name: task_name.to_string(),
            depth: self.depth,
        }
    }

    fn update_history(&self, history: &mut History, task_name: &str) {
        match self.mode {
            Command::Install => history.mark_installed(task_name),
            Command::Update => history.mark_updated(task_name),
            Command::Uninstall => history.mark_uninstalled(task_name),
            Command::List | Command::Validate | Command::Completions { .. } => {}
        }
    }

    fn send(&self, event: TaskEvent) {
        let _ = self.event_tx.send(event);
    }
}

/// Run a task with retry support.
async fn run_task_with_retry(
    name: &str,
    task: &TaskConfig,
    ctx: &CommandContext,
    mode: Command,
    depth: usize,
) -> Result<()> {
    let max_attempts = task.retry + 1;

    for attempt in 1..=max_attempts {
        match run_task(name, task, ctx, mode.clone(), depth).await {
            Ok(()) => return Ok(()),
            Err(e) if attempt < max_attempts => {
                let _ = ctx.event_tx.send(TaskEvent::TaskRetry {
                    task_name: name.to_string(),
                    attempt,
                    max_attempts,
                    error: e.to_string(),
                });
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
            Err(e) => return Err(e),
        }
    }

    unreachable!()
}

async fn run_task(
    name: &str,
    task: &TaskConfig,
    ctx: &CommandContext,
    mode: Command,
    depth: usize,
) -> Result<()> {
    let _ = ctx.event_tx.send(TaskEvent::TaskStarted {
        task_name: name.to_string(),
        command_count: task.commands.len(),
        depth,
    });

    let executors: Vec<Box<dyn CommandExecutor>> =
        task.commands.iter().map(create_executor).collect();

    if task.parallel {
        // Run commands in parallel
        let mut handles = Vec::new();

        for executor in executors {
            let ctx = ctx.clone();
            let mode = mode.clone();
            handles.push(tokio::spawn(async move {
                run_command(executor.as_ref(), &ctx, mode).await
            }));
        }

        for handle in handles {
            handle.await.map_err(|e| Error::Other(e.to_string()))??;
        }
    } else {
        // Run commands sequentially
        for executor in &executors {
            let desc = executor.description();
            let _ = ctx.event_tx.send(TaskEvent::CommandStarted {
                task_name: name.to_string(),
                command_desc: desc.clone(),
            });

            match run_command(executor.as_ref(), ctx, mode.clone()).await {
                Ok(()) => {
                    let _ = ctx.event_tx.send(TaskEvent::CommandCompleted {
                        task_name: name.to_string(),
                        command_desc: desc,
                    });
                }
                Err(e) => {
                    let _ = ctx.event_tx.send(TaskEvent::CommandFailed {
                        task_name: name.to_string(),
                        command_desc: desc,
                        error: e.to_string(),
                    });
                    return Err(e);
                }
            }
        }
    }

    let _ = ctx.event_tx.send(TaskEvent::TaskCompleted {
        task_name: name.to_string(),
    });

    Ok(())
}

async fn run_command(
    executor: &dyn CommandExecutor,
    ctx: &CommandContext,
    mode: Command,
) -> Result<()> {
    match mode {
        Command::Install => executor.install(ctx).await,
        Command::Update => executor.update(ctx).await,
        Command::Uninstall => executor.uninstall(ctx).await,
        Command::List | Command::Validate | Command::Completions { .. } => Ok(()),
    }
}
