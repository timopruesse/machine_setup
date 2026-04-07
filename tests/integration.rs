use std::fs;
use tempfile::tempdir;
use tokio::sync::mpsc;

use machine_setup::cli::Command;
use machine_setup::config;
use machine_setup::engine::event::TaskEvent;
use machine_setup::engine::runner::TaskRunner;

/// Helper: run a config string and collect all events.
async fn run_config(yaml: &str, mode: Command) -> Vec<TaskEvent> {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.yaml");
    fs::write(&config_path, yaml).unwrap();

    let mut config = config::load_config(config_path.to_str().unwrap()).unwrap();
    // Use test-local temp_dir so history doesn't leak between tests
    config.temp_dir = dir.path().join(".ms_temp").to_string_lossy().to_string();

    let (tx, mut rx) = mpsc::unbounded_channel();

    let runner = TaskRunner::new(config, mode, tx).with_config_dir(dir.path().to_path_buf());

    let _ = runner.run_all(true).await;

    // Small yield to let spawned output tasks flush
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Collect all events
    let mut events = Vec::new();
    while let Ok(event) = rx.try_recv() {
        events.push(event);
    }
    events
}

/// Helper: check if events contain a specific pattern.
fn has_event(events: &[TaskEvent], predicate: impl Fn(&TaskEvent) -> bool) -> bool {
    events.iter().any(predicate)
}

fn find_output(events: &[TaskEvent], task: &str, needle: &str) -> bool {
    events.iter().any(|e| {
        matches!(e, TaskEvent::CommandOutput { task_name, line }
            if task_name == task && line.contains(needle))
    })
}

fn task_completed(events: &[TaskEvent], task: &str) -> bool {
    has_event(
        events,
        |e| matches!(e, TaskEvent::TaskCompleted { task_name } if task_name == task),
    )
}

fn task_skipped(events: &[TaskEvent], task: &str) -> bool {
    has_event(
        events,
        |e| matches!(e, TaskEvent::TaskSkipped { task_name, .. } if task_name == task),
    )
}

fn task_failed(events: &[TaskEvent], task: &str) -> bool {
    has_event(
        events,
        |e| matches!(e, TaskEvent::TaskFailed { task_name, .. } if task_name == task),
    )
}

// ─── Run command tests ───

#[tokio::test]
async fn test_run_simple_command() {
    let events = run_config(
        r#"
tasks:
  hello:
    commands:
      - run:
          commands: "echo hello_world"
"#,
        Command::Install,
    )
    .await;

    assert!(task_completed(&events, "hello"));
    assert!(find_output(&events, "hello", "hello_world"));
}

#[tokio::test]
async fn test_run_multiple_commands() {
    let events = run_config(
        r#"
tasks:
  multi:
    commands:
      - run:
          commands:
            - "echo line_one"
            - "echo line_two"
"#,
        Command::Install,
    )
    .await;

    assert!(task_completed(&events, "multi"));
    assert!(find_output(&events, "multi", "line_one"));
    assert!(find_output(&events, "multi", "line_two"));
}

#[tokio::test]
async fn test_run_mode_specific_install() {
    let events = run_config(
        r#"
tasks:
  modes:
    commands:
      - run:
          install: "echo installing"
          update: "echo updating"
          uninstall: "echo removing"
"#,
        Command::Install,
    )
    .await;

    assert!(find_output(&events, "modes", "installing"));
    assert!(!find_output(&events, "modes", "updating"));
    assert!(!find_output(&events, "modes", "removing"));
}

#[tokio::test]
async fn test_run_mode_specific_update() {
    let events = run_config(
        r#"
tasks:
  modes:
    commands:
      - run:
          install: "echo installing"
          update: "echo updating"
"#,
        Command::Update,
    )
    .await;

    assert!(find_output(&events, "modes", "updating"));
    assert!(!find_output(&events, "modes", "installing"));
}

#[tokio::test]
async fn test_run_with_env_vars() {
    let events = run_config(
        r#"
tasks:
  env_test:
    commands:
      - run:
          env:
            MY_VAR: "test_value_123"
          commands: "echo $MY_VAR"
"#,
        Command::Install,
    )
    .await;

    assert!(task_completed(&events, "env_test"));
    // Check the output contains the env var value
    // Note: on some platforms, bash -c with inline export may behave differently
    let has_output = find_output(&events, "env_test", "test_value_123");
    if !has_output {
        eprintln!("env_test events:");
        for e in &events {
            eprintln!("  {e:?}");
        }
    }
    assert!(has_output, "Expected output containing 'test_value_123'");
}

#[tokio::test]
async fn test_run_failing_command() {
    let events = run_config(
        r#"
tasks:
  fail:
    commands:
      - run:
          commands: "exit 1"
"#,
        Command::Install,
    )
    .await;

    assert!(task_failed(&events, "fail"));
}

// ─── OS filter tests ───

#[tokio::test]
async fn test_os_filter_skips_wrong_os() {
    let wrong_os = if cfg!(target_os = "windows") {
        "linux"
    } else {
        "windows"
    };

    let events = run_config(
        &format!(
            r#"
tasks:
  wrong_os:
    os: "{wrong_os}"
    commands:
      - run:
          commands: "echo should_not_run"
"#
        ),
        Command::Install,
    )
    .await;

    assert!(task_skipped(&events, "wrong_os"));
    assert!(!find_output(&events, "wrong_os", "should_not_run"));
}

#[tokio::test]
async fn test_os_filter_runs_current_os() {
    let current_os = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    };

    let events = run_config(
        &format!(
            r#"
tasks:
  right_os:
    os: "{current_os}"
    commands:
      - run:
          commands: "echo correct_os"
"#
        ),
        Command::Install,
    )
    .await;

    assert!(task_completed(&events, "right_os"));
    assert!(find_output(&events, "right_os", "correct_os"));
}

// ─── Copy command tests ───

#[tokio::test]
async fn test_copy_files() {
    let dir = tempdir().unwrap();
    let src_dir = dir.path().join("source");
    let target_dir = dir.path().join("target");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("file.txt"), "hello").unwrap();

    let config_path = dir.path().join("config.yaml");
    fs::write(
        &config_path,
        format!(
            r#"
tasks:
  copy_test:
    commands:
      - copy:
          src: "{}"
          target: "{}"
"#,
            src_dir.to_string_lossy().replace('\\', "/"),
            target_dir.to_string_lossy().replace('\\', "/"),
        ),
    )
    .unwrap();

    let config = config::load_config(config_path.to_str().unwrap()).unwrap();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let runner =
        TaskRunner::new(config, Command::Install, tx).with_config_dir(dir.path().to_path_buf());
    let _ = runner.run_all(true).await;

    // Verify file was copied
    assert!(target_dir.join("file.txt").exists());
    assert_eq!(
        fs::read_to_string(target_dir.join("file.txt")).unwrap(),
        "hello"
    );
}

#[tokio::test]
async fn test_copy_with_ignore() {
    let dir = tempdir().unwrap();
    let src_dir = dir.path().join("source");
    let target_dir = dir.path().join("target");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("keep.txt"), "keep").unwrap();
    fs::write(src_dir.join("ignore.log"), "ignore").unwrap();

    let config_path = dir.path().join("config.yaml");
    fs::write(
        &config_path,
        format!(
            r#"
tasks:
  copy_ignore:
    commands:
      - copy:
          src: "{}"
          target: "{}"
          ignore:
            - "ignore.log"
"#,
            src_dir.to_string_lossy().replace('\\', "/"),
            target_dir.to_string_lossy().replace('\\', "/"),
        ),
    )
    .unwrap();

    let config = config::load_config(config_path.to_str().unwrap()).unwrap();
    let (tx, _rx) = mpsc::unbounded_channel();
    let runner =
        TaskRunner::new(config, Command::Install, tx).with_config_dir(dir.path().to_path_buf());
    let _ = runner.run_all(true).await;

    assert!(target_dir.join("keep.txt").exists());
    assert!(!target_dir.join("ignore.log").exists());
}

// ─── Symlink command tests ───

#[tokio::test]
async fn test_symlink_creation() {
    let dir = tempdir().unwrap();
    let src_dir = dir.path().join("source");
    let target_dir = dir.path().join("target");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("dotfile"), "content").unwrap();

    let config_path = dir.path().join("config.yaml");
    fs::write(
        &config_path,
        format!(
            r#"
tasks:
  symlink_test:
    commands:
      - symlink:
          src: "{}"
          target: "{}"
"#,
            src_dir.to_string_lossy().replace('\\', "/"),
            target_dir.to_string_lossy().replace('\\', "/"),
        ),
    )
    .unwrap();

    let config = config::load_config(config_path.to_str().unwrap()).unwrap();
    let (tx, _rx) = mpsc::unbounded_channel();
    let runner =
        TaskRunner::new(config, Command::Install, tx).with_config_dir(dir.path().to_path_buf());
    let _ = runner.run_all(true).await;

    let link = target_dir.join("dotfile");
    assert!(link.exists() || link.symlink_metadata().is_ok());
}

// ─── History tests ───

#[tokio::test]
async fn test_history_skips_installed() {
    let dir = tempdir().unwrap();
    let temp_dir = dir.path().join(".ms_temp");
    let config_yaml = format!(
        r#"
temp_dir: "{}"
tasks:
  once:
    commands:
      - run:
          commands: "echo installed"
"#,
        temp_dir.to_string_lossy().replace('\\', "/")
    );
    let config_path = dir.path().join("config.yaml");
    fs::write(&config_path, &config_yaml).unwrap();

    // First run
    let config = config::load_config(config_path.to_str().unwrap()).unwrap();
    let (tx1, _rx1) = mpsc::unbounded_channel();
    let runner1 =
        TaskRunner::new(config, Command::Install, tx1).with_config_dir(dir.path().to_path_buf());
    let _ = runner1.run_all(false).await;

    // Second run (should skip)
    let config2 = config::load_config(config_path.to_str().unwrap()).unwrap();
    let (tx2, mut rx2) = mpsc::unbounded_channel();
    let runner2 =
        TaskRunner::new(config2, Command::Install, tx2).with_config_dir(dir.path().to_path_buf());
    let _ = runner2.run_all(false).await;

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let mut events = Vec::new();
    while let Ok(event) = rx2.try_recv() {
        events.push(event);
    }

    assert!(task_skipped(&events, "once"));
}

#[tokio::test]
async fn test_history_force_overrides() {
    let dir = tempdir().unwrap();
    let temp_dir = dir.path().join(".ms_temp");
    let config_yaml = format!(
        r#"
temp_dir: "{}"
tasks:
  forced:
    commands:
      - run:
          commands: "echo forced_run"
"#,
        temp_dir.to_string_lossy().replace('\\', "/")
    );
    let config_path = dir.path().join("config.yaml");
    fs::write(&config_path, &config_yaml).unwrap();

    // First run
    let config = config::load_config(config_path.to_str().unwrap()).unwrap();
    let (tx1, _rx1) = mpsc::unbounded_channel();
    let runner1 =
        TaskRunner::new(config, Command::Install, tx1).with_config_dir(dir.path().to_path_buf());
    let _ = runner1.run_all(false).await;

    // Second run with force
    let config2 = config::load_config(config_path.to_str().unwrap()).unwrap();
    let (tx2, mut rx2) = mpsc::unbounded_channel();
    let runner2 =
        TaskRunner::new(config2, Command::Install, tx2).with_config_dir(dir.path().to_path_buf());
    let _ = runner2.run_all(true).await;

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let mut events = Vec::new();
    while let Ok(event) = rx2.try_recv() {
        events.push(event);
    }

    assert!(task_completed(&events, "forced"));
    assert!(find_output(&events, "forced", "forced_run"));
}

// ─── Parallel execution tests ───

#[tokio::test]
async fn test_parallel_tasks() {
    let events = run_config(
        r#"
parallel: true
tasks:
  a:
    commands:
      - run:
          commands: "echo task_a"
  b:
    commands:
      - run:
          commands: "echo task_b"
"#,
        Command::Install,
    )
    .await;

    assert!(task_completed(&events, "a"));
    assert!(task_completed(&events, "b"));
}

#[tokio::test]
async fn test_parallel_commands_within_task() {
    let events = run_config(
        r#"
tasks:
  parallel_cmds:
    parallel: true
    commands:
      - run:
          commands: "echo cmd_1"
      - run:
          commands: "echo cmd_2"
"#,
        Command::Install,
    )
    .await;

    assert!(task_completed(&events, "parallel_cmds"));
    assert!(find_output(&events, "parallel_cmds", "cmd_1"));
    assert!(find_output(&events, "parallel_cmds", "cmd_2"));
}

// ─── Sub-config composition tests ───

#[tokio::test]
async fn test_machine_setup_sub_config() {
    let dir = tempdir().unwrap();

    // Write sub-config
    fs::write(
        dir.path().join("sub.yaml"),
        r#"
tasks:
  sub_task:
    commands:
      - run:
          commands: "echo from_sub_config"
"#,
    )
    .unwrap();

    // Write main config
    let config_path = dir.path().join("config.yaml");
    fs::write(
        &config_path,
        r#"
tasks:
  include:
    commands:
      - machine_setup:
          config: "./sub.yaml"
"#,
    )
    .unwrap();

    let config = config::load_config(config_path.to_str().unwrap()).unwrap();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let runner =
        TaskRunner::new(config, Command::Install, tx).with_config_dir(dir.path().to_path_buf());
    let _ = runner.run_all(true).await;

    let mut events = Vec::new();
    while let Ok(event) = rx.try_recv() {
        events.push(event);
    }

    assert!(find_output(&events, "sub_task", "from_sub_config"));
}

// ─── Config format tests ───

#[tokio::test]
async fn test_json_config() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.json");
    fs::write(
        &config_path,
        r#"{"tasks": {"json_task": {"commands": [{"run": {"commands": "echo from_json"}}]}}}"#,
    )
    .unwrap();

    let config = config::load_config(config_path.to_str().unwrap()).unwrap();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let runner =
        TaskRunner::new(config, Command::Install, tx).with_config_dir(dir.path().to_path_buf());
    let _ = runner.run_all(true).await;

    let mut events = Vec::new();
    while let Ok(event) = rx.try_recv() {
        events.push(event);
    }

    assert!(task_completed(&events, "json_task"));
    assert!(find_output(&events, "json_task", "from_json"));
}
