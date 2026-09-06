use machine_setup::{cli, config, engine, tui, update_check};

use clap::{CommandFactory, Parser};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use tokio_util::sync::CancellationToken;

use cli::{AddTarget, Cli, Command, RemoveTarget, ReplaceTarget, ScheduleAction};
use config::document_edit::ReplaceOutcome;
use engine::mode::Mode;
use engine::runner::TaskRunner;

/// Tracks temp_dir + config flag for the post-command self update-check.
struct UpdateNoticeCtx {
    temp_dir: PathBuf,
    check_for_updates: bool,
}

impl UpdateNoticeCtx {
    fn default_ctx() -> Self {
        use machine_setup::utils::path::expand_path;
        Self {
            temp_dir: expand_path("~/.machine_setup", None),
            check_for_updates: true,
        }
    }

    fn from_config(app_config: &config::types::AppConfig) -> Self {
        use machine_setup::utils::path::expand_path;
        Self {
            temp_dir: expand_path(&app_config.temp_dir, None),
            check_for_updates: app_config.check_for_updates,
        }
    }

    fn emit(&self, command: &Command) {
        update_check::maybe_print_update_notice(command, &self.temp_dir, self.check_for_updates);
    }
}

fn main() -> anyhow::Result<()> {
    // Detached update-check worker — no clap, no notice, exit when cache is written.
    if std::env::var_os(update_check::ENV_INTERNAL_REFRESH).is_some() {
        update_check::run_internal_refresh_worker();
        return Ok(());
    }

    let cli = Cli::parse();
    let mut notice = UpdateNoticeCtx::default_ctx();

    // Handle completions (no config needed) — skipped by update_check
    if let Command::Completions { shell } = &cli.command {
        let mut cmd = Cli::command();
        clap_complete::generate(*shell, &mut cmd, "machine_setup", &mut std::io::stdout());
        return Ok(());
    }

    // Schema dump — skipped by update_check
    if cli.command == Command::Schema {
        let schema = config::schema::generate_pretty()?;
        println!("{schema}");
        return Ok(());
    }

    // Shell-hook target: avoid Config locate/load (hot path on every new shell).
    if let Command::Schedule {
        action: ScheduleAction::Notify { ref temp_dir },
    } = &cli.command
    {
        let dir = resolve_notify_temp_dir(temp_dir.as_deref(), cli.config.as_deref())?;
        if let Some(msg) = machine_setup::schedule::notices::notify(&dir)? {
            println!("{msg}");
        }
        return Ok(());
    }

    let cwd = std::env::current_dir().unwrap_or_default();

    // Authoring verbs mutate the Config document
    if cli.command == Command::Init {
        let path = config::document::resolve_init_path(cli.config.as_deref(), &cwd);
        config::document::init(&path)?;
        println!("Created {}", path.display());
        if config::document::validate_after_write(&path)? {
            notice.emit(&cli.command);
            std::process::exit(1);
        }
        notice.emit(&cli.command);
        return Ok(());
    }

    if cli.command == Command::Wizard {
        config::wizard::run(cli.config.as_deref(), &cwd)?;
        notice.emit(&cli.command);
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
                let emitted = config::recipes::emit_from_cli(recipe)?;
                let name = emitted.name.clone();
                config::document::append_emitted(&path, &emitted)?;
                println!("Added recipe task `{name}` to {}", path.display());
            }
        }
        if config::document::validate_after_write(&path)? {
            notice.emit(&cli.command);
            std::process::exit(1);
        }
        notice.emit(&cli.command);
        return Ok(());
    }

    if let Command::Remove { target } = &cli.command {
        let path = resolve_existing_document(cli.config.as_deref(), &cwd)?;
        match target {
            RemoveTarget::Task { name, fix_deps } => {
                let mode = if *fix_deps {
                    config::document_edit::FixDepsMode::Force
                } else {
                    config::document_edit::FixDepsMode::Auto
                };
                config::document_edit::remove_task(&path, name, mode)?;
                println!("Removed task `{name}` from {}", path.display());
            }
        }
        if config::document::validate_after_write(&path)? {
            notice.emit(&cli.command);
            std::process::exit(1);
        }
        notice.emit(&cli.command);
        return Ok(());
    }

    if let Command::Replace { target } = &cli.command {
        let path = resolve_existing_document(cli.config.as_deref(), &cwd)?;
        let emitted = match target {
            ReplaceTarget::Task { name } => config::document::emitted_blank_task(name)?,
            ReplaceTarget::Recipe { recipe } => config::recipes::emit_from_cli(recipe)?,
        };
        let outcome = config::document_edit::replace_task(
            &path,
            &emitted,
            config::document_edit::ReplaceMode::Auto,
        )?;
        match outcome {
            ReplaceOutcome::Created => {
                eprintln!(
                    "warning: task `{}` did not exist; created it in {}",
                    emitted.name,
                    path.display()
                );
            }
            ReplaceOutcome::Replaced => {
                println!("Replaced task `{}` in {}", emitted.name, path.display());
            }
        }
        if config::document::validate_after_write(&path)? {
            notice.emit(&cli.command);
            std::process::exit(1);
        }
        notice.emit(&cli.command);
        return Ok(());
    }

    // Load config (supports local paths, URLs, and locator when `-c` omitted)
    let config_source = config::resolve_config_source(cli.config.as_deref(), &cwd)?;
    let app_config = config::load_config(&config_source)?;
    notice = UpdateNoticeCtx::from_config(&app_config);

    // Handle list command
    if cli.command == Command::List {
        use machine_setup::tui::catalog::{adapt, plain, run_browse};

        let history = config::history::History::load(&notice.temp_dir).unwrap_or_default();
        let items = adapt::list_items(&app_config, &history);
        let use_tui = !cli.no_tui && std::io::stdout().is_terminal();
        if use_tui {
            run_browse(items, None)?;
        } else {
            plain::print_list(&items);
        }
        notice.emit(&cli.command);
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
                notice.emit(&cli.command);
                std::process::exit(1);
            }
        }
        notice.emit(&cli.command);
        return Ok(());
    }

    if let Command::Doctor { fix } = cli.command {
        let config_dir = config::resolve_config_dir(&config_source, &cwd);
        let use_tui = !cli.no_tui && std::io::stdout().is_terminal();
        run_doctor(&app_config, &config_dir, fix, use_tui)?;
        notice.emit(&cli.command);
        return Ok(());
    }

    if let Command::Schedule { action } = &cli.command {
        run_schedule(
            &app_config,
            &config_source,
            &cwd,
            action.clone(),
            cli.no_tui,
        )?;
        notice.emit(&cli.command);
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
        notice.emit(&cli.command);
        return Ok(());
    }

    // All non-execution verbs returned above.
    #[expect(
        clippy::expect_used,
        reason = "non-execution verbs return before this point"
    )]
    let mode = Mode::from_command(&cli.command)
        .expect("non-execution verbs are handled before this point");

    let interactive = std::io::stdin().is_terminal() && !cli.no_tui;
    let task_names =
        match resolve_selected_tasks(&app_config, seed, mode, cli.with_deps, interactive)? {
            Some(names) => names,
            None => {
                println!("Aborted.");
                notice.emit(&cli.command);
                return Ok(());
            }
        };

    // Execution verbs only: boot a multi-thread runtime here, not for sync verbs above.
    let command = cli.command.clone();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let result = rt.block_on(run_execution(cli, app_config, config_source, task_names));
    notice.emit(&command);
    result
}

/// Path for add/list-style ops that need an existing local Config document.
fn resolve_existing_document(
    config_arg: Option<&str>,
    cwd: &Path,
) -> anyhow::Result<std::path::PathBuf> {
    if let Some(raw) = config_arg {
        if config::is_url(raw) {
            anyhow::bail!(
                "`add`/`remove`/`replace` require a local Config document path, not a URL"
            );
        }
        let path = config::resolve_config_path(Path::new(raw))?;
        return Ok(path);
    }
    Ok(config::locator::find(cwd)?)
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

    if use_tui && !cli.dry_run && app_config.requires_sudo(&task_names) {
        pre_authenticate_sudo();
    }

    #[expect(
        clippy::expect_used,
        reason = "non-execution verbs return before this point"
    )]
    let mode = Mode::from_command(&cli.command)
        .expect("non-execution verbs are handled before this point");

    let runner = TaskRunner::new(app_config, mode, events)
        .with_config_dir(config_dir)
        .with_cancel(cancel.clone())
        .with_dry_run(cli.dry_run)
        .with_backup(cli.backup);
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

        if cli.dry_run {
            println!("=== DRY-RUN MODE: No changes will be applied ===\n");
        }

        let consumer = tokio::spawn(tui::plain::run(event_rx));

        let result = tokio::select! {
            result = run_engine(runner, &task_names, force) => result,
            _ = cancel.cancelled() => {
                eprintln!("\nInterrupted.");
                Err(machine_setup::error::Error::Aborted)
            }
        };

        let _ = consumer.await;
        result?;
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

fn run_schedule(
    app_config: &config::types::AppConfig,
    config_source: &str,
    cwd: &Path,
    action: ScheduleAction,
    _no_tui: bool,
) -> anyhow::Result<()> {
    use machine_setup::schedule::{apply, run, status};
    use machine_setup::utils::path::expand_path;

    let temp_dir = expand_path(&app_config.temp_dir, None);
    let config_path = Path::new(config_source)
        .canonicalize()
        .unwrap_or_else(|_| Path::new(config_source).to_path_buf());
    let binary = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("machine_setup"));

    match action {
        ScheduleAction::Apply { no_install_hook } => {
            let report = apply::apply(
                app_config,
                &config_path,
                &temp_dir,
                &binary,
                !no_install_hook,
            )?;
            println!(
                "Applied {} schedule unit(s): {}",
                report.keys.len(),
                if report.keys.is_empty() {
                    "(none)".into()
                } else {
                    report
                        .keys
                        .iter()
                        .zip(report.labels.iter())
                        .map(|(k, l)| format!("{k} → {l}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                }
            );
            if let Some(hook) = report.hook_script {
                println!("Hook script: {}", hook.display());
            }
            if report.stubs_updated.is_empty() {
                if !no_install_hook {
                    println!(
                        "No ~/.zshrc or ~/.bashrc found to update; source the hook script manually if desired."
                    );
                }
            } else {
                for p in report.stubs_updated {
                    println!("Updated shell stub in {}", p.display());
                }
            }
        }
        ScheduleAction::Remove { keep_hook } => {
            let report = apply::remove(&temp_dir, keep_hook)?;
            println!(
                "Removed {} unit(s): {}",
                report.removed_labels.len(),
                if report.removed_labels.is_empty() {
                    "(none)".into()
                } else {
                    report.removed_labels.join(", ")
                }
            );
            for p in report.stubs_cleared {
                println!("Cleared shell stub in {}", p.display());
            }
        }
        ScheduleAction::Run { key } => {
            let config_dir = config::resolve_config_dir(config_source, cwd);
            let key = run::parse_key_arg(&key)?;
            let rt = tokio::runtime::Runtime::new()?;
            let report = rt.block_on(run::run_key(
                app_config.clone(),
                config_dir,
                &key,
                &temp_dir,
            ))?;
            if report.skipped_not_installed {
                println!(
                    "No installed tasks for schedule {} — nothing to update.",
                    report.key
                );
            } else {
                if !report.updated.is_empty() {
                    println!("Updated: {}", report.updated.join(", "));
                }
                if !report.failed.is_empty() {
                    println!("Failed: {}", report.failed.join(", "));
                    anyhow::bail!("schedule run had failing tasks");
                }
            }
        }
        ScheduleAction::Status => {
            print!("{}", status::render_status(app_config, &temp_dir)?);
        }
        ScheduleAction::Notify { .. } => {
            unreachable!("schedule notify is handled before config load");
        }
    }
    Ok(())
}

/// Temp dir for `schedule notify`: `--temp-dir`, else Config `-c` `temp_dir`, else default.
fn resolve_notify_temp_dir(
    explicit: Option<&Path>,
    config_arg: Option<&str>,
) -> anyhow::Result<PathBuf> {
    use machine_setup::utils::path::expand_path;

    if let Some(p) = explicit {
        let raw = p.to_string_lossy();
        return Ok(expand_path(raw.as_ref(), None));
    }
    if let Some(raw) = config_arg {
        let cwd = std::env::current_dir().unwrap_or_default();
        let source = config::resolve_config_source(Some(raw), &cwd)?;
        let app_config = config::load_config(&source)?;
        return Ok(expand_path(&app_config.temp_dir, None));
    }
    Ok(expand_path("~/.machine_setup", None))
}

fn run_doctor(
    config: &config::types::AppConfig,
    config_dir: &Path,
    fix: bool,
    use_tui: bool,
) -> anyhow::Result<()> {
    use config::status::{doctor, prune_orphans};
    use machine_setup::tui::catalog::{adapt, plain, run_browse};
    use machine_setup::utils::path::expand_path;

    let temp_dir = expand_path(&config.temp_dir, None);
    let mut history = config::history::History::load(&temp_dir).unwrap_or_default();
    let report = doctor(config, &history, config_dir);

    let items = adapt::doctor_items(&report);
    let banner = adapt::doctor_banner(&report, fix);
    let issue_lines: Vec<String> = report
        .issues
        .iter()
        .map(|i| format!("[{}] {}: {}", i.severity, i.task_name, i.message))
        .collect();

    if use_tui {
        run_browse(items, Some(banner))?;
    } else {
        plain::print_doctor(&items, &issue_lines, &report.orphans);
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
    } else if !use_tui && !report.orphans.is_empty() {
        println!("\nHint: re-run with `doctor --fix` to remove orphan History entries.");
    }

    if has_errors {
        anyhow::bail!("doctor found validation errors");
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
