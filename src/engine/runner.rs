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
        }
    }

    pub fn with_config_dir(mut self, dir: PathBuf) -> Self {
        self.config_dir = dir;
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
        let temp_dir = expand_path(&self.config.temp_dir, None);
        let mut history = History::load(&temp_dir).unwrap_or_default();

        let mut succeeded = 0usize;
        let mut failed = 0usize;
        let mut skipped = 0usize;

        if self.config.parallel {
            // Parallel execution
            let mut handles = Vec::new();

            for name in task_names {
                let task_config = &self.config.tasks[name];

                // Check OS filter
                if !task_config.os.matches_current() {
                    self.send(TaskEvent::TaskSkipped {
                        task_name: name.clone(),
                        reason: "OS mismatch".to_string(),
                    });
                    skipped += 1;
                    continue;
                }

                // Check history
                if self.mode == Command::Install && !force && history.is_installed(name) {
                    self.send(TaskEvent::TaskSkipped {
                        task_name: name.clone(),
                        reason: "Already installed (use --force to reinstall)".to_string(),
                    });
                    skipped += 1;
                    continue;
                }

                let ctx = self.create_context(name, &temp_dir);
                let task = task_config.clone();
                let mode = self.mode;
                let name = name.clone();
                handles.push(tokio::spawn(async move {
                    let result = run_task(&name, &task, &ctx, mode).await;
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
        } else {
            // Sequential execution
            for name in task_names {
                let task_config = &self.config.tasks[name];

                // Check OS filter
                if !task_config.os.matches_current() {
                    self.send(TaskEvent::TaskSkipped {
                        task_name: name.clone(),
                        reason: "OS mismatch".to_string(),
                    });
                    skipped += 1;
                    continue;
                }

                // Check history
                if self.mode == Command::Install && !force && history.is_installed(name) {
                    self.send(TaskEvent::TaskSkipped {
                        task_name: name.clone(),
                        reason: "Already installed (use --force to reinstall)".to_string(),
                    });
                    skipped += 1;
                    continue;
                }

                let ctx = self.create_context(name, &temp_dir);

                match run_task(name, task_config, &ctx, self.mode).await {
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
            mode: self.mode,
            config_dir: self.config_dir.clone(),
            temp_dir: temp_dir.to_path_buf(),
            default_shell: self.config.default_shell.clone(),
            task_name: task_name.to_string(),
        }
    }

    fn update_history(&self, history: &mut History, task_name: &str) {
        match self.mode {
            Command::Install => history.mark_installed(task_name),
            Command::Update => history.mark_updated(task_name),
            Command::Uninstall => history.mark_uninstalled(task_name),
            Command::List => {}
        }
    }

    fn send(&self, event: TaskEvent) {
        let _ = self.event_tx.send(event);
    }
}

async fn run_task(
    name: &str,
    task: &TaskConfig,
    ctx: &CommandContext,
    mode: Command,
) -> Result<()> {
    let _ = ctx.event_tx.send(TaskEvent::TaskStarted {
        task_name: name.to_string(),
        command_count: task.commands.len(),
    });

    let executors: Vec<Box<dyn CommandExecutor>> =
        task.commands.iter().map(create_executor).collect();

    if task.parallel {
        // Run commands in parallel
        let mut handles = Vec::new();

        for executor in executors {
            let ctx = ctx.clone();
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

            match run_command(executor.as_ref(), ctx, mode).await {
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
        Command::List => Ok(()),
    }
}
