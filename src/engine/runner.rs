use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::config::graph::TaskGraph;
use crate::config::history::History;
use crate::config::types::{AppConfig, TaskConfig};
use crate::error::{Error, Result};
use crate::utils::path::expand_path;

use super::commands::{catalog, create_executor, CommandExecutor, Executor};
use super::concurrency::{ConcurrencyGate, ExclusiveLane};
use super::conditions::evaluate_skip;
use super::context::CommandContext;
use super::event::TaskEvent;
use super::mode::Mode;
use super::sink::{SharedSink, TaskEventSink};

pub struct TaskRunner {
    config: AppConfig,
    mode: Mode,
    events: SharedSink,
    gate: Arc<ConcurrencyGate>,
    config_dir: Arc<PathBuf>,
    depth: usize,
    /// Lazily built executors per task name — one `Arc<Executor>` per Command entry.
    executor_cache: Mutex<HashMap<String, Arc<[Arc<Executor>]>>>,
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
            config_dir: Arc::new(std::env::current_dir().unwrap_or_default()),
            depth: 0,
            executor_cache: Mutex::new(HashMap::new()),
        }
    }

    /// Override the concurrency gate (e.g. nested Sub-config sharing the parent).
    pub fn with_gate(mut self, gate: Arc<ConcurrencyGate>) -> Self {
        self.gate = gate;
        self
    }

    pub fn with_config_dir(mut self, dir: PathBuf) -> Self {
        self.config_dir = Arc::new(dir);
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

        let temp_dir = Arc::new(expand_path(&self.config.temp_dir, None));
        let mut history = History::load(temp_dir.as_path()).unwrap_or_default();

        // Both execution modes are the same loop over layers; sequential is the
        // degenerate case where each task is its own layer (so the join below
        // runs them one at a time, in dependency order).
        let mut layers: Vec<Vec<String>> = if self.config.parallel {
            graph.layers(ordered)
        } else {
            ordered.iter().map(|name| vec![name.clone()]).collect()
        };

        // Uninstall dependents before dependencies: reverse the layer list
        // (do not re-layer after reversing names — that would collapse layers).
        if self.mode == Mode::Uninstall {
            layers.reverse();
        }

        let mut tally = Tally::default();
        for layer in &layers {
            self.run_layer(
                layer,
                force,
                Arc::clone(&temp_dir),
                &mut history,
                &mut tally,
            )
            .await;
        }

        // Save history
        if let Err(e) = history.save(temp_dir.as_path()) {
            tracing::warn!("Failed to save history: {e}");
        }

        self.send(TaskEvent::AllDone {
            succeeded: tally.succeeded,
            failed: tally.failed,
            skipped: tally.skipped,
        });

        if tally.failed > 0 {
            Err(Error::TasksFailed(tally.failed))
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
        temp_dir: Arc<PathBuf>,
        history: &mut History,
        tally: &mut Tally,
    ) {
        let mut handles = Vec::new();

        for name in layer {
            let task_config = Arc::clone(&self.config.tasks[name]);

            if let Some(reason) = evaluate_skip(
                task_config.as_ref(),
                name,
                self.mode,
                force,
                history,
                self.config_dir.as_path(),
                &self.config.default_shell,
            ) {
                self.send(TaskEvent::TaskSkipped {
                    task_name: Arc::<str>::from(name.as_str()),
                    reason,
                });
                tally.skipped += 1;
                continue;
            }

            let ctx = self.create_context(name, Arc::clone(&temp_dir));
            let name = Arc::clone(&ctx.task_name);
            let executors = self.executors_for_task(name.as_ref(), task_config.as_ref());
            handles.push(tokio::spawn(async move {
                let result = run_task_with_retry(&task_config, &ctx, executors).await;
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

    /// Get ordered task names (for list command / TUI display).
    #[allow(dead_code)]
    pub fn task_names(&self) -> Vec<String> {
        self.config.tasks.keys().cloned().collect()
    }

    /// Get task configs for display.
    #[allow(dead_code)]
    pub fn tasks(&self) -> &indexmap::IndexMap<String, Arc<TaskConfig>> {
        &self.config.tasks
    }

    fn create_context(&self, task_name: &str, temp_dir: Arc<PathBuf>) -> CommandContext {
        CommandContext {
            events: Arc::clone(&self.events),
            gate: Arc::clone(&self.gate),
            mode: self.mode,
            config_dir: Arc::clone(&self.config_dir),
            temp_dir,
            default_shell: self.config.default_shell.clone(),
            task_name: Arc::<str>::from(task_name),
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

    /// Return cached executors for `task_name`, building them on first access.
    fn executors_for_task(&self, task_name: &str, task: &TaskConfig) -> Arc<[Arc<Executor>]> {
        #[expect(
            clippy::expect_used,
            reason = "executor cache mutex is never poisoned in normal operation"
        )]
        let mut cache = self
            .executor_cache
            .lock()
            .expect("executor cache mutex is never poisoned in normal operation");
        if let Some(cached) = cache.get(task_name) {
            return Arc::clone(cached);
        }
        let executors: Arc<[Arc<Executor>]> = task
            .commands
            .iter()
            .map(|entry| Arc::new(create_executor(entry)))
            .collect::<Vec<_>>()
            .into();
        cache.insert(task_name.to_string(), Arc::clone(&executors));
        executors
    }

    #[cfg(test)]
    fn executor_cache_len(&self) -> usize {
        #[expect(
            clippy::expect_used,
            reason = "executor cache mutex is never poisoned in normal operation"
        )]
        self.executor_cache
            .lock()
            .expect("executor cache mutex is never poisoned in normal operation")
            .len()
    }
}

/// Run a task with retry support.
async fn run_task_with_retry(
    task: &TaskConfig,
    ctx: &CommandContext,
    executors: Arc<[Arc<Executor>]>,
) -> Result<()> {
    let max_attempts = task.retry + 1;

    for attempt in 1..=max_attempts {
        match run_task(task, ctx, Arc::clone(&executors)).await {
            Ok(()) => return Ok(()),
            Err(e) if attempt < max_attempts => {
                ctx.emit(TaskEvent::TaskRetry {
                    task_name: Arc::clone(&ctx.task_name),
                    attempt,
                    max_attempts,
                    error: e.to_string(),
                });
                tokio::time::sleep(std::time::Duration::from_secs(task.retry_delay_secs)).await;
            }
            Err(e) => return Err(e),
        }
    }

    unreachable!()
}

async fn run_task(
    task: &TaskConfig,
    ctx: &CommandContext,
    executors: Arc<[Arc<Executor>]>,
) -> Result<()> {
    ctx.emit(TaskEvent::TaskStarted {
        task_name: Arc::clone(&ctx.task_name),
        command_count: task.commands.len(),
        depth: ctx.depth,
    });

    let command_total = task.commands.len();

    if task.parallel {
        let mut handles = Vec::new();

        for (i, entry) in task.commands.iter().enumerate() {
            let command_index = i + 1;
            let executor = Arc::clone(&executors[i]);
            let desc = catalog::description(entry);
            let lane = catalog::exclusive_lane(entry, ctx.mode);
            ctx.emit(TaskEvent::CommandStarted {
                task_name: Arc::clone(&ctx.task_name),
                command_desc: desc.clone(),
                command_index,
                command_total,
            });
            let ctx = ctx.clone();
            handles.push(tokio::spawn(async move {
                let result =
                    execute_with_gate(lane, &executor, &ctx, &desc, command_index, command_total)
                        .await;
                (desc, command_index, result)
            }));
        }

        for handle in handles {
            let (desc, command_index, result) =
                handle.await.map_err(|e| Error::TaskJoin(e.to_string()))?;
            match result {
                Ok(()) => {
                    ctx.emit(TaskEvent::CommandCompleted {
                        task_name: Arc::clone(&ctx.task_name),
                        command_desc: desc,
                        command_index,
                        command_total,
                    });
                }
                Err(e) => {
                    ctx.emit(TaskEvent::CommandFailed {
                        task_name: Arc::clone(&ctx.task_name),
                        command_desc: desc,
                        command_index,
                        command_total,
                        error: e.to_string(),
                    });
                    return Err(e);
                }
            }
        }
    } else {
        for (i, entry) in task.commands.iter().enumerate() {
            let command_index = i + 1;
            let executor = &executors[i];
            let desc = catalog::description(entry);
            let lane = catalog::exclusive_lane(entry, ctx.mode);
            ctx.emit(TaskEvent::CommandStarted {
                task_name: Arc::clone(&ctx.task_name),
                command_desc: desc.clone(),
                command_index,
                command_total,
            });

            match execute_with_gate(lane, executor, ctx, &desc, command_index, command_total).await
            {
                Ok(()) => {
                    ctx.emit(TaskEvent::CommandCompleted {
                        task_name: Arc::clone(&ctx.task_name),
                        command_desc: desc,
                        command_index,
                        command_total,
                    });
                }
                Err(e) => {
                    ctx.emit(TaskEvent::CommandFailed {
                        task_name: Arc::clone(&ctx.task_name),
                        command_desc: desc,
                        command_index,
                        command_total,
                        error: e.to_string(),
                    });
                    return Err(e);
                }
            }
        }
    }

    ctx.emit(TaskEvent::TaskCompleted {
        task_name: Arc::clone(&ctx.task_name),
    });

    Ok(())
}

/// Admit a Command entry: Exclusive lane (if any) first, then work permit.
async fn execute_with_gate(
    lane: Option<ExclusiveLane>,
    executor: &Executor,
    ctx: &CommandContext,
    command_desc: &str,
    command_index: usize,
    command_total: usize,
) -> Result<()> {
    let _lane_permit = if let Some(lane) = lane {
        match ctx.gate.try_acquire_lane(lane) {
            Some(permit) => Some(permit),
            None => {
                ctx.emit(TaskEvent::CommandWaiting {
                    task_name: ctx.task_name.clone(),
                    command_desc: command_desc.to_string(),
                    command_index,
                    command_total,
                    lane,
                });
                Some(ctx.gate.acquire_lane(lane).await)
            }
        }
    } else {
        None
    };

    if executor.occupies_concurrency_slot() {
        let _permit = ctx.gate.acquire().await;
        executor.execute(ctx).await
    } else {
        executor.execute(ctx).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::{CommandEntry, RunArgs, Shell, StringOrVec, TaskConfig};
    use crate::engine::sink::NullSink;
    use indexmap::IndexMap;
    use std::collections::HashMap;
    use tempfile::tempdir;

    fn run_entry(cmd: &str) -> CommandEntry {
        CommandEntry::Run(RunArgs {
            commands: cmd.into(),
            install: StringOrVec::default(),
            update: StringOrVec::default(),
            uninstall: StringOrVec::default(),
            shell: None,
            env: HashMap::new(),
            quiet: false,
        })
    }

    fn task_with(commands: Vec<CommandEntry>, parallel: bool) -> TaskConfig {
        TaskConfig {
            commands,
            os: Default::default(),
            parallel,
            only_if: Default::default(),
            skip_if: Default::default(),
            depends_on: Default::default(),
            retry: 0,
            retry_delay_secs: 1,
            auto_update: None,
        }
    }

    fn make_config(tasks: IndexMap<String, TaskConfig>) -> AppConfig {
        AppConfig {
            tasks: tasks
                .into_iter()
                .map(|(name, task)| (name, Arc::new(task)))
                .collect(),
            temp_dir: "~/.machine_setup".to_string(),
            default_shell: Shell::Bash,
            parallel: false,
            num_threads: None,
            check_for_updates: true,
        }
    }

    #[test]
    fn executors_for_task_caches_same_arc_pointers() {
        let mut tasks = IndexMap::new();
        tasks.insert(
            "demo".to_string(),
            task_with(vec![run_entry("echo one"), run_entry("echo two")], false),
        );
        let runner = TaskRunner::new(make_config(tasks), Mode::Install, NullSink::shared());

        let first = runner.executors_for_task("demo", runner.config.tasks["demo"].as_ref());
        let second = runner.executors_for_task("demo", runner.config.tasks["demo"].as_ref());

        assert_eq!(first.len(), 2);
        assert!(Arc::ptr_eq(&first[0], &second[0]));
        assert!(Arc::ptr_eq(&first[1], &second[1]));
        assert_eq!(runner.executor_cache_len(), 1);
    }

    #[tokio::test]
    async fn run_populates_executor_cache_and_runs_parallel_and_sequential() {
        let dir = tempdir().unwrap();
        let mut tasks = IndexMap::new();
        tasks.insert(
            "sequential".to_string(),
            task_with(vec![run_entry("echo seq")], false),
        );
        tasks.insert(
            "parallel".to_string(),
            task_with(vec![run_entry("echo one"), run_entry("echo two")], true),
        );
        let mut config = make_config(tasks);
        config.temp_dir = dir.path().join(".ms_temp").to_string_lossy().to_string();

        let runner = TaskRunner::new(config, Mode::Install, NullSink::shared())
            .with_config_dir(dir.path().to_path_buf());

        runner.run_all(true).await.unwrap();

        assert_eq!(runner.executor_cache_len(), 2);
    }
}
