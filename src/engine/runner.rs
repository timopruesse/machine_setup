use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::config::graph::TaskGraph;
use crate::config::history::History;
use crate::config::types::{AppConfig, TaskConfig};
use crate::error::{Error, Result};
use crate::utils::path::expand_path;

use super::commands::{create_executor, CommandExecutor};
use super::concurrency::ConcurrencyGate;
use super::context::CommandContext;
use super::event::TaskEvent;
use super::mode::Mode;
use super::sink::{SharedSink, TaskEventSink};

pub struct TaskRunner {
    config: AppConfig,
    mode: Mode,
    events: SharedSink,
    gate: Arc<ConcurrencyGate>,
    config_dir: PathBuf,
    depth: usize,
}

/// Running counts of task outcomes across all layers of a run.
#[derive(Default)]
struct Tally {
    succeeded: usize,
    failed: usize,
    skipped: usize,
}

impl TaskRunner {
    pub fn new(config: AppConfig, mode: Mode, events: SharedSink) -> Self {
        let gate = Arc::new(ConcurrencyGate::from_num_threads(config.num_threads));
        Self {
            config,
            mode,
            events,
            gate,
            config_dir: std::env::current_dir().unwrap_or_default(),
            depth: 0,
        }
    }

    /// Override the concurrency gate (e.g. nested Sub-config sharing the parent).
    pub fn with_gate(mut self, gate: Arc<ConcurrencyGate>) -> Self {
        self.gate = gate;
        self
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
        // Resolve dependency order via the task graph. When no task has
        // dependencies, this borrows `task_names` instead of cloning it.
        let graph = TaskGraph::new(&self.config.tasks);
        let ordered = graph.topo_order(task_names)?;
        let ordered: &[String] = &ordered;

        let temp_dir = expand_path(&self.config.temp_dir, None);
        let mut history = History::load(&temp_dir).unwrap_or_default();

        // Both execution modes are the same loop over layers; sequential is the
        // degenerate case where each task is its own layer (so the join below
        // runs them one at a time, in dependency order).
        let layers: Vec<Vec<String>> = if self.config.parallel {
            graph.layers(ordered)
        } else {
            ordered.iter().map(|name| vec![name.clone()]).collect()
        };

        let mut tally = Tally::default();
        for layer in &layers {
            self.run_layer(layer, force, &temp_dir, &mut history, &mut tally)
                .await;
        }

        // Save history
        if let Err(e) = history.save(&temp_dir) {
            tracing::warn!("Failed to save history: {e}");
        }

        self.send(TaskEvent::AllDone {
            succeeded: tally.succeeded,
            failed: tally.failed,
            skipped: tally.skipped,
        });

        if tally.failed > 0 {
            Err(Error::Other(format!("{} task(s) failed", tally.failed)))
        } else {
            Ok(())
        }
    }

    /// Run one dependency layer: skip what should be skipped, spawn the rest,
    /// then join — recording each task's outcome into `tally` and history. A
    /// layer of one task (sequential mode) runs that task to completion before
    /// the caller advances to the next layer.
    ///
    /// ConcurrencyGate permits are acquired per Command entry inside
    /// [`run_task`] (not per Task), so nested Sub-configs can share the gate
    /// without deadlocking (ADR-0003).
    async fn run_layer(
        &self,
        layer: &[String],
        force: bool,
        temp_dir: &Path,
        history: &mut History,
        tally: &mut Tally,
    ) {
        let mut handles = Vec::new();

        for name in layer {
            let task_config = &self.config.tasks[name];

            if let Some(reason) = self.should_skip(task_config, name, force, history) {
                self.send(TaskEvent::TaskSkipped {
                    task_name: name.clone(),
                    reason,
                });
                tally.skipped += 1;
                continue;
            }

            let ctx = self.create_context(name, temp_dir);
            let task = task_config.clone();
            let name = name.clone();
            handles.push(tokio::spawn(async move {
                let result = run_task_with_retry(&name, &task, &ctx).await;
                (name, result)
            }));
        }

        for handle in handles {
            match handle.await {
                Ok((name, Ok(()))) => {
                    self.update_history(history, &name);
                    tally.succeeded += 1;
                }
                Ok((name, Err(e))) => {
                    self.send(TaskEvent::TaskFailed {
                        task_name: name,
                        error: e.to_string(),
                    });
                    tally.failed += 1;
                }
                Err(_) => {
                    tally.failed += 1;
                }
            }
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
        if self.mode == Mode::Install && !force && history.is_installed(name) {
            return Some("Already installed (use --force to reinstall)".to_string());
        }

        None
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
            events: Arc::clone(&self.events),
            gate: Arc::clone(&self.gate),
            mode: self.mode,
            config_dir: self.config_dir.clone(),
            temp_dir: temp_dir.to_path_buf(),
            default_shell: self.config.default_shell.clone(),
            task_name: task_name.to_string(),
            depth: self.depth,
        }
    }

    fn update_history(&self, history: &mut History, task_name: &str) {
        match self.mode {
            Mode::Install => history.mark_installed(task_name),
            Mode::Update => history.mark_updated(task_name),
            Mode::Uninstall => history.mark_uninstalled(task_name),
        }
    }

    fn send(&self, event: TaskEvent) {
        TaskEventSink::emit(self.events.as_ref(), event);
    }
}

/// Run a task with retry support.
async fn run_task_with_retry(name: &str, task: &TaskConfig, ctx: &CommandContext) -> Result<()> {
    let max_attempts = task.retry + 1;

    for attempt in 1..=max_attempts {
        match run_task(name, task, ctx).await {
            Ok(()) => return Ok(()),
            Err(e) if attempt < max_attempts => {
                ctx.emit(TaskEvent::TaskRetry {
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

async fn run_task(name: &str, task: &TaskConfig, ctx: &CommandContext) -> Result<()> {
    ctx.emit(TaskEvent::TaskStarted {
        task_name: name.to_string(),
        command_count: task.commands.len(),
        depth: ctx.depth,
    });

    let executors: Vec<Box<dyn CommandExecutor>> =
        task.commands.iter().cloned().map(create_executor).collect();

    if task.parallel {
        let mut handles = Vec::new();

        for executor in executors {
            let ctx = ctx.clone();
            handles.push(tokio::spawn(async move {
                execute_with_gate(executor.as_ref(), &ctx).await
            }));
        }

        for handle in handles {
            handle.await.map_err(|e| Error::Other(e.to_string()))??;
        }
    } else {
        for executor in &executors {
            let desc = executor.description();
            ctx.emit(TaskEvent::CommandStarted {
                task_name: name.to_string(),
                command_desc: desc.clone(),
            });

            match execute_with_gate(executor.as_ref(), ctx).await {
                Ok(()) => {
                    ctx.emit(TaskEvent::CommandCompleted {
                        task_name: name.to_string(),
                        command_desc: desc,
                    });
                }
                Err(e) => {
                    ctx.emit(TaskEvent::CommandFailed {
                        task_name: name.to_string(),
                        command_desc: desc,
                        error: e.to_string(),
                    });
                    return Err(e);
                }
            }
        }
    }

    ctx.emit(TaskEvent::TaskCompleted {
        task_name: name.to_string(),
    });

    Ok(())
}

/// Acquire a ConcurrencyGate permit for leaf Command entries only.
async fn execute_with_gate(executor: &dyn CommandExecutor, ctx: &CommandContext) -> Result<()> {
    if executor.occupies_concurrency_slot() {
        let permit = ctx.gate.acquire().await;
        let _permit = permit;
        executor.execute(ctx).await
    } else {
        executor.execute(ctx).await
    }
}
