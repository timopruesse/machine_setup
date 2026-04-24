use machine_setup::{cli, config, engine, error, tui};

use clap::{CommandFactory, Parser};
use std::io::IsTerminal;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use cli::{Cli, Command};
use engine::event::TaskEvent;
use engine::runner::TaskRunner;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Handle completions (no config needed)
    if let Command::Completions { shell } = &cli.command {
        let mut cmd = Cli::command();
        clap_complete::generate(*shell, &mut cmd, "machine_setup", &mut std::io::stdout());
        return Ok(());
    }

    // Load config (supports local paths and URLs)
    let app_config = config::load_config(&cli.config)?;

    // Handle list command
    if cli.command == Command::List {
        print_task_list(&app_config);
        return Ok(());
    }

    // Handle validate command
    if cli.command == Command::Validate {
        let config_dir = std::path::Path::new(&cli.config)
            .canonicalize()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

        let issues = config::validate::validate_config(&app_config, &config_dir);
        if issues.is_empty() {
            println!("Config is valid.");
        } else {
            let has_errors = issues
                .iter()
                .any(|i| matches!(i.severity, config::validate::Severity::Error));
            for issue in &issues {
                println!(
                    "[{}] {}: {}",
                    issue.severity, issue.task_name, issue.message
                );
            }
            if has_errors {
                std::process::exit(1);
            }
        }
        return Ok(());
    }

    // Determine which tasks to run (interactive selection must happen before TUI starts)
    let task_names: Vec<String> = if let Some(ref task_name) = cli.task {
        vec![task_name.clone()]
    } else if cli.select {
        select_tasks(&app_config)?
    } else {
        app_config.tasks.keys().cloned().collect()
    };

    if task_names.is_empty() {
        println!("No tasks selected.");
        return Ok(());
    }

    // Resolve config directory for relative paths (URLs fall back to cwd)
    let config_dir = std::path::Path::new(&cli.config)
        .canonicalize()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    // Create event channel and cancellation token
    let (event_tx, event_rx) = mpsc::unbounded_channel::<TaskEvent>();
    let cancel = CancellationToken::new();

    // Determine if we should use the TUI
    let use_tui = !cli.no_tui && std::io::stdout().is_terminal();

    // Pre-authenticate sudo before TUI takes over the terminal
    if use_tui && app_config.requires_sudo(&task_names) {
        pre_authenticate_sudo();
    }

    // Set up runner (moves app_config)
    let runner = TaskRunner::new(app_config, cli.command.clone(), event_tx)
        .with_config_dir(config_dir);
    let force = cli.force;
    let task_names_clone = task_names.clone();

    if use_tui {
        // Spawn engine in background
        let engine_cancel = cancel.clone();
        let engine_handle = tokio::spawn(async move {
            tokio::select! {
                result = run_engine(runner, &task_names_clone, force) => result,
                _ = engine_cancel.cancelled() => {
                    Ok(()) // Cancelled by user
                }
            }
        });

        // Run TUI in foreground (blocks until user quits)
        tui::run(event_rx, task_names, cli.command, cancel).await?;

        // Abort engine if still running
        engine_handle.abort();
        let _ = engine_handle.await;
    } else {
        // Plain mode: set up logging
        let log_level = if cli.debug {
            "debug"
        } else {
            cli.level.as_str()
        };
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(log_level)),
            )
            .init();

        // Handle Ctrl+C in plain mode
        let plain_cancel = cancel.clone();
        tokio::spawn(async move {
            let _ = tokio::signal::ctrl_c().await;
            plain_cancel.cancel();
        });

        // Spawn plain consumer
        let consumer = tokio::spawn(tui::plain::run(event_rx));

        // Run engine
        let result = tokio::select! {
            result = run_engine(runner, &task_names, force) => result,
            _ = cancel.cancelled() => {
                eprintln!("\nInterrupted.");
                Ok(())
            }
        };

        // Wait for consumer to drain
        drop(result);
        let _ = consumer.await;
    }

    Ok(())
}

async fn run_engine(
    runner: TaskRunner,
    task_names: &[String],
    force: bool,
) -> crate::error::Result<()> {
    if task_names.len() == 1 {
        runner.run_single_task(&task_names[0], force).await
    } else {
        runner.run_tasks(task_names, force).await
    }
}

fn print_task_list(config: &config::types::AppConfig) {
    println!("Defined tasks:\n");
    for (name, task) in &config.tasks {
        let os_info = match &task.os {
            config::os::OsFilter::All => "all".to_string(),
            config::os::OsFilter::Single(os) => os.to_string(),
            config::os::OsFilter::Multiple(oses) => oses
                .iter()
                .map(|o| o.to_string())
                .collect::<Vec<_>>()
                .join(", "),
        };
        let parallel = if task.parallel { " [parallel]" } else { "" };
        println!("  {name} (os: {os_info}){parallel}");

        for cmd in &task.commands {
            println!("    - {cmd}");
        }
        println!();
    }
}

/// Run `sudo -v` to cache credentials before the TUI takes over stdin.
/// This way sudo commands inside tasks won't hang waiting for a password prompt.
fn pre_authenticate_sudo() {
    #[cfg(unix)]
    {
        use std::process::Command as StdCommand;

        // Check if sudo is available
        if StdCommand::new("sudo")
            .arg("-n")
            .arg("true")
            .status()
            .is_ok_and(|s| s.success())
        {
            // Already authenticated (passwordless or cached), no prompt needed
            return;
        }

        eprintln!("Some tasks require sudo. Please enter your password:");
        let _ = StdCommand::new("sudo")
            .arg("-v")
            .stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status();
    }
}

fn select_tasks(config: &config::types::AppConfig) -> anyhow::Result<Vec<String>> {
    let task_names: Vec<String> = config.tasks.keys().cloned().collect();

    let selections = dialoguer::MultiSelect::new()
        .with_prompt("Select tasks to run")
        .items(&task_names)
        .interact()?;

    Ok(selections
        .into_iter()
        .map(|i| task_names[i].clone())
        .collect())
}
