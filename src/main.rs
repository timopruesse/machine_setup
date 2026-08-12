use machine_setup::{cli, config, engine, tui};

use clap::{CommandFactory, Parser};
use std::io::IsTerminal;
use std::path::Path;
use tokio_util::sync::CancellationToken;

use cli::{AddTarget, Cli, Command, RecipeCommand};
use engine::mode::Mode;
use engine::runner::TaskRunner;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Handle completions (no config needed)
    if let Command::Completions { shell } = &cli.command {
        let mut cmd = Cli::command();
        clap_complete::generate(*shell, &mut cmd, "machine_setup", &mut std::io::stdout());
        return Ok(());
    }

    // Schema dump (no Config document needed)
    if cli.command == Command::Schema {
        let schema = config::schema::generate_pretty()?;
        println!("{schema}");
        return Ok(());
    }

    let cwd = std::env::current_dir().unwrap_or_default();

    // Authoring verbs mutate the Config document
    if cli.command == Command::Init {
        let path = config::document::resolve_init_path(cli.config.as_deref(), &cwd);
        config::document::init(&path)?;
        println!("Created {}", path.display());
        if config::document::validate_after_write(&path)? {
            std::process::exit(1);
        }
        return Ok(());
    }

    if cli.command == Command::Wizard {
        config::wizard::run(cli.config.as_deref(), &cwd)?;
        return Ok(());
    }

    if let Command::Add { target } = &cli.command {
        let path = resolve_existing_document(cli.config.as_deref(), &cwd)?;
        match target {
            AddTarget::Task { name } => {
                config::document::add_task(&path, name)?;
                println!("Added task `{name}` to {}", path.display());
            }
            AddTarget::Recipe { recipe } => {
                let emitted = emit_recipe(recipe)?;
                let name = emitted.name.clone();
                config::document::append_emitted(&path, &emitted)?;
                println!("Added recipe task `{name}` to {}", path.display());
            }
        }
        if config::document::validate_after_write(&path)? {
            std::process::exit(1);
        }
        return Ok(());
    }

    // Load config (supports local paths, URLs, and locator when `-c` omitted)
    let config_source = config::resolve_config_source(cli.config.as_deref(), &cwd)?;
    let app_config = config::load_config(&config_source)?;

    // Handle list command
    if cli.command == Command::List {
        use machine_setup::tui::catalog::{adapt, plain, run_browse};
        use machine_setup::utils::path::expand_path;

        let temp_dir = expand_path(&app_config.temp_dir, None);
        let history = config::history::History::load(&temp_dir).unwrap_or_default();
        let items = adapt::list_items(&app_config, &history);
        let use_tui = !cli.no_tui && std::io::stdout().is_terminal();
        if use_tui {
            run_browse(items)?;
        } else {
            plain::print_list(&items);
        }
        return Ok(());
    }

    // Handle validate command
    if cli.command == Command::Validate {
        let config_dir = config::resolve_config_dir(&config_source, &cwd);

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

    if let Command::Doctor { fix } = cli.command {
        let config_dir = config::resolve_config_dir(&config_source, &cwd);
        run_doctor(&app_config, &config_dir, fix)?;
        return Ok(());
    }

    // Determine which tasks to run (interactive selection must happen before TUI starts)
    let seed: Vec<String> = if let Some(ref task_name) = cli.task {
        vec![task_name.clone()]
    } else if cli.select {
        let use_tui = !cli.no_tui && std::io::stdout().is_terminal();
        select_tasks(&app_config, use_tui)?
    } else {
        app_config.tasks.keys().cloned().collect()
    };

    if seed.is_empty() {
        println!("No tasks selected.");
        return Ok(());
    }

    // All non-execution verbs returned above.
    let mode = Mode::from_command(&cli.command)
        .expect("non-execution verbs are handled before this point");

    let interactive = std::io::stdin().is_terminal() && !cli.no_tui;
    let task_names =
        match resolve_selected_tasks(&app_config, seed, mode, cli.with_deps, interactive)? {
            Some(names) => names,
            None => {
                println!("Aborted.");
                return Ok(());
            }
        };

    // Execution verbs only: boot a multi-thread runtime here, not for sync verbs above.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    rt.block_on(run_execution(cli, app_config, config_source, task_names))
}

/// Path for add/list-style ops that need an existing local Config document.
fn resolve_existing_document(
    config_arg: Option<&str>,
    cwd: &Path,
) -> anyhow::Result<std::path::PathBuf> {
    if let Some(raw) = config_arg {
        if config::is_url(raw) {
            anyhow::bail!("`add` requires a local Config document path, not a URL");
        }
        let path = config::resolve_config_path(Path::new(raw))?;
        return Ok(path);
    }
    Ok(config::locator::find(cwd)?)
}

fn emit_recipe(recipe: &RecipeCommand) -> anyhow::Result<config::recipes::EmittedTask> {
    use config::recipes::{
        emit_brew_bundle, emit_dotfiles, emit_git_repo, BrewBundleParams, DotfilesParams,
        GitRepoParams,
    };

    Ok(match recipe {
        RecipeCommand::Dotfiles {
            url,
            src,
            target,
            ignore,
            name,
        } => {
            let ignore_refs: Vec<&str> = ignore.iter().map(String::as_str).collect();
            emit_dotfiles(&DotfilesParams {
                name,
                url,
                src,
                target,
                ignore: ignore_refs,
            })?
        }
        RecipeCommand::GitRepo { url, target, name } => {
            emit_git_repo(&GitRepoParams { name, url, target })?
        }
        RecipeCommand::BrewBundle { file, name } => {
            emit_brew_bundle(&BrewBundleParams { name, file })?
        }
    })
}

async fn run_execution(
    cli: Cli,
    app_config: config::types::AppConfig,
    config_source: String,
    task_names: Vec<String>,
) -> anyhow::Result<()> {
    let cwd = std::env::current_dir().unwrap_or_default();
    let config_dir = config::resolve_config_dir(&config_source, &cwd);

    let (events, event_rx) = engine::sink::ChannelSink::channel();
    let cancel = CancellationToken::new();

    let use_tui = !cli.no_tui && std::io::stdout().is_terminal();

    if use_tui && app_config.requires_sudo(&task_names) {
        pre_authenticate_sudo();
    }

    let mode = Mode::from_command(&cli.command)
        .expect("non-execution verbs are handled before this point");

    let runner = TaskRunner::new(app_config, mode, events).with_config_dir(config_dir);
    let force = cli.force;
    let task_names_clone = task_names.clone();

    if use_tui {
        let engine_cancel = cancel.clone();
        let engine_handle = tokio::spawn(async move {
            tokio::select! {
                result = run_engine(runner, &task_names_clone, force) => result,
                _ = engine_cancel.cancelled() => {
                    Ok(())
                }
            }
        });

        tui::run(event_rx, task_names, mode, cancel).await?;

        engine_handle.abort();
        let _ = engine_handle.await;
    } else {
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

        let plain_cancel = cancel.clone();
        tokio::spawn(async move {
            let _ = tokio::signal::ctrl_c().await;
            plain_cancel.cancel();
        });

        let consumer = tokio::spawn(tui::plain::run(event_rx));

        let result = tokio::select! {
            result = run_engine(runner, &task_names, force) => result,
            _ = cancel.cancelled() => {
                eprintln!("\nInterrupted.");
                Ok(())
            }
        };

        drop(result);
        let _ = consumer.await;
    }

    Ok(())
}

async fn run_engine(
    runner: TaskRunner,
    task_names: &[String],
    force: bool,
) -> machine_setup::error::Result<()> {
    if task_names.len() == 1 {
        runner.run_single_task(&task_names[0], force).await
    } else {
        runner.run_tasks(task_names, force).await
    }
}

fn run_doctor(
    config: &config::types::AppConfig,
    config_dir: &Path,
    fix: bool,
) -> anyhow::Result<()> {
    use config::status::{doctor, format_ts, os_label, prune_orphans};
    use machine_setup::utils::path::expand_path;

    let temp_dir = expand_path(&config.temp_dir, None);
    let mut history = config::history::History::load(&temp_dir).unwrap_or_default();
    let report = doctor(config, &history, config_dir);

    println!("Tasks:\n");
    for row in &report.rows {
        let installed = if row.installed { "yes" } else { "no" };
        let os_note = if row.os_applies {
            String::new()
        } else {
            " [os: skipped on this host]".to_string()
        };
        let (installed_at, updated_at) = match row.history {
            Some(h) => (format_ts(h.installed_at), format_ts(h.updated_at)),
            None => ("-".into(), "-".into()),
        };
        println!(
            "  {} (os: {}, installed: {installed}, installed_at: {installed_at}, updated_at: {updated_at}){os_note}",
            row.name,
            os_label(&row.task.os),
        );
    }

    println!("\nValidation:");
    if report.issues.is_empty() {
        println!("  Config is valid.");
    } else {
        for issue in &report.issues {
            println!(
                "  [{}] {}: {}",
                issue.severity, issue.task_name, issue.message
            );
        }
    }

    println!("\nHistory orphans:");
    if report.orphans.is_empty() {
        println!("  none");
    } else {
        for name in &report.orphans {
            println!("  {name} (in History, not in Config document)");
        }
    }

    let has_errors = report.has_errors();

    if fix {
        let removed = prune_orphans(&mut history, config);
        if removed.is_empty() {
            println!("\n--fix: no orphan History entries to remove.");
        } else {
            history.save(&temp_dir)?;
            println!(
                "\n--fix: removed {} orphan History entr{}: {}",
                removed.len(),
                if removed.len() == 1 { "y" } else { "ies" },
                removed.join(", ")
            );
        }
    } else if !report.orphans.is_empty() {
        println!("\nHint: re-run with `doctor --fix` to remove orphan History entries.");
    }

    if has_errors {
        std::process::exit(1);
    }
    Ok(())
}

/// Run `sudo -v` to cache credentials before the TUI takes over stdin.
fn pre_authenticate_sudo() {
    #[cfg(unix)]
    {
        use std::process::Command as StdCommand;

        if StdCommand::new("sudo")
            .arg("-n")
            .arg("true")
            .status()
            .is_ok_and(|s| s.success())
        {
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

fn select_tasks(config: &config::types::AppConfig, use_tui: bool) -> anyhow::Result<Vec<String>> {
    if use_tui {
        use machine_setup::tui::catalog::{adapt, run_select};
        use machine_setup::utils::path::expand_path;

        let temp_dir = expand_path(&config.temp_dir, None);
        let history = config::history::History::load(&temp_dir).unwrap_or_default();
        let items = adapt::select_items(config, &history);
        match run_select(items)? {
            Some(ids) => Ok(ids),
            None => Ok(vec![]),
        }
    } else if std::io::stdin().is_terminal() {
        let mut task_names: Vec<String> = config.tasks.keys().cloned().collect();

        let selections = dialoguer::MultiSelect::new()
            .with_prompt("Select tasks to run")
            .items(&task_names)
            .interact()?;

        Ok(selections
            .into_iter()
            .map(|i| std::mem::take(&mut task_names[i]))
            .collect())
    } else {
        anyhow::bail!("cannot select tasks interactively (no TTY); omit -s or pass -t")
    }
}

/// Apply mode-aware dependency expansion / uninstall prompts.
fn resolve_selected_tasks(
    config: &config::types::AppConfig,
    selected: Vec<String>,
    mode: Mode,
    with_deps: bool,
    interactive: bool,
) -> anyhow::Result<Option<Vec<String>>> {
    use config::selection;

    let tasks = if mode == Mode::Uninstall && !with_deps && interactive {
        let candidates = selection::uninstall_dep_candidates(config, &selected)?;
        if candidates.is_empty() {
            selected
        } else {
            let picks = dialoguer::MultiSelect::new()
                .with_prompt("Also uninstall dependencies?")
                .items(&candidates)
                .interact()?;
            let extras: Vec<String> = picks.into_iter().map(|i| candidates[i].clone()).collect();
            selection::apply_extra_deps(selected, extras)
        }
    } else {
        selection::expand_for_mode(config, &selected, mode, with_deps)?
    };

    if mode == Mode::Uninstall {
        let warnings = selection::shared_dep_warnings(config, &tasks);
        if !warnings.is_empty() {
            eprintln!("Warning: uninstalling tasks that other tasks still depend on:");
            for (task, dependents) in &warnings {
                eprintln!("  '{task}' is depended on by: {}", dependents.join(", "));
            }
            if interactive {
                let proceed = dialoguer::Confirm::new()
                    .with_prompt("Continue uninstalling anyway?")
                    .default(false)
                    .interact()?;
                if !proceed {
                    return Ok(None);
                }
            }
        }
    }

    Ok(Some(tasks))
}
