use std::fs;
use tempfile::tempdir;

use machine_setup::config;
use machine_setup::engine::event::TaskEvent;
use machine_setup::engine::mode::Mode;
use machine_setup::engine::runner::TaskRunner;

/// Helper: run a config string and collect all events.
async fn run_config(yaml: &str, mode: Mode) -> Vec<TaskEvent> {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.yaml");
    fs::write(&config_path, yaml).unwrap();

    let mut config = config::load_config(config_path.to_str().unwrap()).unwrap();
    // Use test-local temp_dir so history doesn't leak between tests
    config.temp_dir = dir.path().join(".ms_temp").to_string_lossy().to_string();

    let (events, mut rx) = machine_setup::engine::sink::ChannelSink::channel();

    let runner = TaskRunner::new(config, mode, events).with_config_dir(dir.path().to_path_buf());

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

/// Run only the named tasks (no selection-time expansion — caller expands).
async fn run_named_tasks(yaml: &str, mode: Mode, names: &[&str]) -> Vec<TaskEvent> {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.yaml");
    fs::write(&config_path, yaml).unwrap();

    let mut config = config::load_config(config_path.to_str().unwrap()).unwrap();
    config.temp_dir = dir.path().join(".ms_temp").to_string_lossy().to_string();

    let (events, mut rx) = machine_setup::engine::sink::ChannelSink::channel();
    let runner = TaskRunner::new(config, mode, events).with_config_dir(dir.path().to_path_buf());

    let task_names: Vec<String> = names.iter().map(|s| s.to_string()).collect();
    let _ = runner.run_tasks(&task_names, true).await;

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

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
        Mode::Install,
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
        Mode::Install,
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
        Mode::Install,
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
        Mode::Update,
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
        Mode::Install,
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
        Mode::Install,
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
        Mode::Install,
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
        Mode::Install,
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
    let (events, _rx) = machine_setup::engine::sink::ChannelSink::channel();
    let runner =
        TaskRunner::new(config, Mode::Install, events).with_config_dir(dir.path().to_path_buf());
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
    let (events, _rx) = machine_setup::engine::sink::ChannelSink::channel();
    let runner =
        TaskRunner::new(config, Mode::Install, events).with_config_dir(dir.path().to_path_buf());
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
    let (events, _rx) = machine_setup::engine::sink::ChannelSink::channel();
    let runner =
        TaskRunner::new(config, Mode::Install, events).with_config_dir(dir.path().to_path_buf());
    let _ = runner.run_all(true).await;

    let link = target_dir.join("dotfile");
    assert!(link.exists() || link.symlink_metadata().is_ok());
}

#[tokio::test]
async fn test_symlink_unwraps_nested_dir_symlink_without_corrupting_source() {
    let dir = tempdir().unwrap();
    let src_dir = dir.path().join("src");
    let target_dir = dir.path().join("target");
    let src_pack = src_dir.join("skills").join("route-agents");
    let target_skills = target_dir.join("skills");
    let skill_body = "route-agents-body";

    fs::create_dir_all(&src_pack).unwrap();
    fs::write(src_pack.join("SKILL.md"), skill_body).unwrap();
    fs::create_dir_all(&target_skills).unwrap();

    #[cfg(unix)]
    std::os::unix::fs::symlink(&src_pack, target_skills.join("route-agents")).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(&src_pack, target_skills.join("route-agents")).unwrap();

    assert!(target_skills
        .join("route-agents")
        .symlink_metadata()
        .unwrap()
        .file_type()
        .is_symlink());

    let config_path = dir.path().join("config.yaml");
    fs::write(
        &config_path,
        format!(
            r#"
tasks:
  symlink_unwrap:
    commands:
      - symlink:
          src: "{}"
          target: "{}"
          force: true
"#,
            src_dir.to_string_lossy().replace('\\', "/"),
            target_dir.to_string_lossy().replace('\\', "/"),
        ),
    )
    .unwrap();

    let config = config::load_config(config_path.to_str().unwrap()).unwrap();
    let (events, _rx) = machine_setup::engine::sink::ChannelSink::channel();
    let runner =
        TaskRunner::new(config, Mode::Install, events).with_config_dir(dir.path().to_path_buf());
    let _ = runner.run_all(true).await;

    let dest_pack = target_skills.join("route-agents");
    let dest_meta = dest_pack
        .symlink_metadata()
        .expect("dest pack should exist");
    assert!(
        dest_meta.is_dir() && !dest_meta.file_type().is_symlink(),
        "dest pack must be a real directory, not a leftover dir symlink"
    );

    let dest_skill = dest_pack.join("SKILL.md");
    assert!(
        dest_skill
            .symlink_metadata()
            .unwrap()
            .file_type()
            .is_symlink(),
        "dest SKILL.md should be a symlink"
    );

    let src_skill = src_pack.join("SKILL.md");
    let src_meta = src_skill.symlink_metadata().expect("source SKILL.md");
    assert!(
        src_meta.is_file() && !src_meta.file_type().is_symlink(),
        "source SKILL.md must remain a regular file (not a self-symlink)"
    );
    assert_eq!(fs::read_to_string(&src_skill).unwrap(), skill_body);
}

#[tokio::test]
async fn test_symlink_unwraps_nested_dir_symlink_without_force_for_new_leaf() {
    let dir = tempdir().unwrap();
    let src_dir = dir.path().join("src");
    let target_dir = dir.path().join("target");
    let src_pack = src_dir.join("skills").join("route-agents");
    let target_skills = target_dir.join("skills");

    fs::create_dir_all(&src_pack).unwrap();
    fs::write(src_pack.join("SKILL.md"), "body").unwrap();
    fs::write(src_pack.join("NEW.md"), "new-leaf").unwrap();
    fs::create_dir_all(&target_skills).unwrap();

    #[cfg(unix)]
    std::os::unix::fs::symlink(&src_pack, target_skills.join("route-agents")).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(&src_pack, target_skills.join("route-agents")).unwrap();

    let config_path = dir.path().join("config.yaml");
    fs::write(
        &config_path,
        format!(
            r#"
tasks:
  symlink_unwrap_noforce:
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
    let (events, _rx) = machine_setup::engine::sink::ChannelSink::channel();
    let runner =
        TaskRunner::new(config, Mode::Install, events).with_config_dir(dir.path().to_path_buf());
    let _ = runner.run_all(true).await;

    let dest_pack = target_skills.join("route-agents");
    assert!(!dest_pack
        .symlink_metadata()
        .unwrap()
        .file_type()
        .is_symlink());
    assert!(dest_pack
        .join("NEW.md")
        .symlink_metadata()
        .unwrap()
        .file_type()
        .is_symlink());
    assert!(!src_pack
        .join("NEW.md")
        .symlink_metadata()
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(
        fs::read_to_string(src_pack.join("NEW.md")).unwrap(),
        "new-leaf"
    );
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
    let (events1, _rx1) = machine_setup::engine::sink::ChannelSink::channel();
    let runner1 =
        TaskRunner::new(config, Mode::Install, events1).with_config_dir(dir.path().to_path_buf());
    let _ = runner1.run_all(false).await;

    // Second run (should skip)
    let config2 = config::load_config(config_path.to_str().unwrap()).unwrap();
    let (events2, mut rx2) = machine_setup::engine::sink::ChannelSink::channel();
    let runner2 =
        TaskRunner::new(config2, Mode::Install, events2).with_config_dir(dir.path().to_path_buf());
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
    let (events1, _rx1) = machine_setup::engine::sink::ChannelSink::channel();
    let runner1 =
        TaskRunner::new(config, Mode::Install, events1).with_config_dir(dir.path().to_path_buf());
    let _ = runner1.run_all(false).await;

    // Second run with force
    let config2 = config::load_config(config_path.to_str().unwrap()).unwrap();
    let (events2, mut rx2) = machine_setup::engine::sink::ChannelSink::channel();
    let runner2 =
        TaskRunner::new(config2, Mode::Install, events2).with_config_dir(dir.path().to_path_buf());
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
        Mode::Install,
    )
    .await;

    assert!(task_completed(&events, "a"));
    assert!(task_completed(&events, "b"));
}

#[tokio::test]
async fn test_num_threads_one_still_runs_parallel_tasks() {
    // Concurrency gate with limit 1 must serialize Tasks but still complete.
    let events = run_config(
        r#"
default_shell: bash
parallel: true
num_threads: 1
tasks:
  a:
    commands:
      - run:
          commands: "echo a-done"
  b:
    commands:
      - run:
          commands: "echo b-done"
"#,
        Mode::Install,
    )
    .await;

    assert!(task_completed(&events, "a"));
    assert!(task_completed(&events, "b"));
    assert!(find_output(&events, "a", "a-done"));
    assert!(find_output(&events, "b", "b-done"));
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
        Mode::Install,
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
    let (events, mut rx) = machine_setup::engine::sink::ChannelSink::channel();
    let runner =
        TaskRunner::new(config, Mode::Install, events).with_config_dir(dir.path().to_path_buf());
    let _ = runner.run_all(true).await;

    let mut events = Vec::new();
    while let Ok(event) = rx.try_recv() {
        events.push(event);
    }

    assert!(find_output(&events, "sub_task", "from_sub_config"));
}

#[tokio::test]
async fn test_nested_sub_config_with_num_threads_one() {
    // Regression: shared ConcurrencyGate must not deadlock when the parent
    // machine_setup command would otherwise hold the only permit.
    let dir = tempdir().unwrap();
    fs::write(
        dir.path().join("sub.yaml"),
        r#"
tasks:
  nested:
    commands:
      - run:
          commands: "echo nested-ok"
"#,
    )
    .unwrap();

    let config_path = dir.path().join("config.yaml");
    fs::write(
        &config_path,
        r#"
default_shell: bash
num_threads: 1
tasks:
  parent:
    commands:
      - machine_setup:
          config: "./sub.yaml"
"#,
    )
    .unwrap();

    let config = config::load_config(config_path.to_str().unwrap()).unwrap();
    let (events, mut rx) = machine_setup::engine::sink::ChannelSink::channel();
    let runner =
        TaskRunner::new(config, Mode::Install, events).with_config_dir(dir.path().to_path_buf());

    let run = tokio::time::timeout(std::time::Duration::from_secs(5), runner.run_all(true)).await;
    assert!(
        run.is_ok(),
        "nested sub-config deadlocked under num_threads: 1"
    );
    assert!(run.unwrap().is_ok());

    let mut events = Vec::new();
    while let Ok(event) = rx.try_recv() {
        events.push(event);
    }
    assert!(find_output(&events, "nested", "nested-ok"));
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
    let (events, mut rx) = machine_setup::engine::sink::ChannelSink::channel();
    let runner =
        TaskRunner::new(config, Mode::Install, events).with_config_dir(dir.path().to_path_buf());
    let _ = runner.run_all(true).await;

    let mut events = Vec::new();
    while let Ok(event) = rx.try_recv() {
        events.push(event);
    }

    assert!(task_completed(&events, "json_task"));
    assert!(find_output(&events, "json_task", "from_json"));
}

// ─── Security tests ───

#[tokio::test]
async fn test_env_var_injection_prevented() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.yaml");
    fs::write(
        &config_path,
        r#"
tasks:
  injection_test:
    commands:
      - run:
          env:
            MY_VAR: "$(echo injected)"
          commands: "echo $MY_VAR"
"#,
    )
    .unwrap();

    let config = config::load_config(config_path.to_str().unwrap()).unwrap();
    let (events, mut rx) = machine_setup::engine::sink::ChannelSink::channel();
    let runner =
        TaskRunner::new(config, Mode::Install, events).with_config_dir(dir.path().to_path_buf());
    let _ = runner.run_all(true).await;

    let mut events = Vec::new();
    while let Ok(event) = rx.try_recv() {
        events.push(event);
    }

    assert!(task_completed(&events, "injection_test"));
    // The value should be the literal string, not the result of command substitution
    assert!(find_output(&events, "injection_test", "$(echo injected)"));
    assert!(!find_output(&events, "injection_test", "injected\n"));
}

// ─── Validation tests ───

#[tokio::test]
async fn test_validate_catches_invalid_env_key() {
    use machine_setup::config::validate;

    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.yaml");
    fs::write(
        &config_path,
        r#"
tasks:
  bad_env:
    commands:
      - run:
          env:
            "INVALID-KEY": "value"
          commands: "echo test"
"#,
    )
    .unwrap();

    let config = config::load_config(config_path.to_str().unwrap()).unwrap();
    let issues = validate::validate_config(&config, dir.path());
    assert!(issues
        .iter()
        .any(|i| i.message.contains("INVALID-KEY")
            && matches!(i.severity, validate::Severity::Error)));
}

#[tokio::test]
async fn test_validate_reports_empty_task() {
    use machine_setup::config::validate;

    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.yaml");
    fs::write(
        &config_path,
        r#"
tasks:
  empty_task:
    commands: []
"#,
    )
    .unwrap();

    let config = config::load_config(config_path.to_str().unwrap()).unwrap();
    let issues = validate::validate_config(&config, dir.path());
    assert!(issues.iter().any(|i| i.task_name == "empty_task"
        && i.message.contains("no commands")
        && matches!(i.severity, validate::Severity::Warning)));
}

// ─── Conditional task tests (only_if / skip_if) ───

#[tokio::test]
async fn test_only_if_path_exists_runs() {
    let dir = tempdir().unwrap();
    let marker = dir.path().join("marker_file");
    fs::write(&marker, "").unwrap();

    let config_path = dir.path().join("config.yaml");
    fs::write(
        &config_path,
        format!(
            r#"
tasks:
  conditional:
    only_if: "{}"
    commands:
      - run:
          commands: "echo only_if_ran"
"#,
            marker.to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();

    let config = config::load_config(config_path.to_str().unwrap()).unwrap();
    let (events, mut rx) = machine_setup::engine::sink::ChannelSink::channel();
    let runner =
        TaskRunner::new(config, Mode::Install, events).with_config_dir(dir.path().to_path_buf());
    let _ = runner.run_all(true).await;

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let mut events = Vec::new();
    while let Ok(event) = rx.try_recv() {
        events.push(event);
    }

    assert!(task_completed(&events, "conditional"));
    assert!(find_output(&events, "conditional", "only_if_ran"));
}

#[tokio::test]
async fn test_only_if_path_missing_skips() {
    let events = run_config(
        r#"
tasks:
  conditional:
    only_if: "/nonexistent/path/that/should/not/exist"
    commands:
      - run:
          commands: "echo should_not_run"
"#,
        Mode::Install,
    )
    .await;

    assert!(task_skipped(&events, "conditional"));
    assert!(!find_output(&events, "conditional", "should_not_run"));
}

#[tokio::test]
async fn test_skip_if_path_exists_skips() {
    let dir = tempdir().unwrap();
    let marker = dir.path().join("skip_marker");
    fs::write(&marker, "").unwrap();

    let config_path = dir.path().join("config.yaml");
    fs::write(
        &config_path,
        format!(
            r#"
tasks:
  conditional:
    skip_if: "{}"
    commands:
      - run:
          commands: "echo should_not_run"
"#,
            marker.to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();

    let config = config::load_config(config_path.to_str().unwrap()).unwrap();
    let (events, mut rx) = machine_setup::engine::sink::ChannelSink::channel();
    let runner =
        TaskRunner::new(config, Mode::Install, events).with_config_dir(dir.path().to_path_buf());
    let _ = runner.run_all(true).await;

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let mut events = Vec::new();
    while let Ok(event) = rx.try_recv() {
        events.push(event);
    }

    assert!(task_skipped(&events, "conditional"));
}

#[tokio::test]
async fn test_skip_if_path_missing_runs() {
    let events = run_config(
        r#"
tasks:
  conditional:
    skip_if: "/nonexistent/path/that/should/not/exist"
    commands:
      - run:
          commands: "echo skip_if_ran"
"#,
        Mode::Install,
    )
    .await;

    assert!(task_completed(&events, "conditional"));
    assert!(find_output(&events, "conditional", "skip_if_ran"));
}

// ─── Dependency ordering tests (depends_on) ───

#[tokio::test]
async fn test_depends_on_ordering() {
    let events = run_config(
        r#"
tasks:
  second:
    depends_on: ["first"]
    commands:
      - run:
          commands: "echo second_task"
  first:
    commands:
      - run:
          commands: "echo first_task"
"#,
        Mode::Install,
    )
    .await;

    assert!(task_completed(&events, "first"));
    assert!(task_completed(&events, "second"));

    // Verify first completed before second started
    let first_done = events
        .iter()
        .position(|e| matches!(e, TaskEvent::TaskCompleted { task_name } if task_name == "first"));
    let second_start = events.iter().position(
        |e| matches!(e, TaskEvent::TaskStarted { task_name, .. } if task_name == "second"),
    );
    assert!(first_done.is_some());
    assert!(second_start.is_some());
    assert!(first_done.unwrap() < second_start.unwrap());
}

#[tokio::test]
async fn test_depends_on_transitive() {
    let events = run_config(
        r#"
tasks:
  c:
    depends_on: ["b"]
    commands:
      - run:
          commands: "echo task_c"
  b:
    depends_on: ["a"]
    commands:
      - run:
          commands: "echo task_b"
  a:
    commands:
      - run:
          commands: "echo task_a"
"#,
        Mode::Install,
    )
    .await;

    assert!(task_completed(&events, "a"));
    assert!(task_completed(&events, "b"));
    assert!(task_completed(&events, "c"));

    // Verify order: a before b, b before c
    let a_done = events
        .iter()
        .position(|e| matches!(e, TaskEvent::TaskCompleted { task_name } if task_name == "a"))
        .unwrap();
    let b_start = events
        .iter()
        .position(|e| matches!(e, TaskEvent::TaskStarted { task_name, .. } if task_name == "b"))
        .unwrap();
    let b_done = events
        .iter()
        .position(|e| matches!(e, TaskEvent::TaskCompleted { task_name } if task_name == "b"))
        .unwrap();
    let c_start = events
        .iter()
        .position(|e| matches!(e, TaskEvent::TaskStarted { task_name, .. } if task_name == "c"))
        .unwrap();

    assert!(a_done < b_start);
    assert!(b_done < c_start);
}

#[tokio::test]
async fn test_update_selected_task_does_not_run_deps() {
    let events = run_named_tasks(
        r#"
tasks:
  dep:
    commands:
      - run:
          update: "echo dep_ran"
  leaf:
    depends_on: ["dep"]
    commands:
      - run:
          update: "echo leaf_ran"
"#,
        Mode::Update,
        &["leaf"],
    )
    .await;

    assert!(task_completed(&events, "leaf"));
    assert!(find_output(&events, "leaf", "leaf_ran"));
    assert!(!task_completed(&events, "dep"));
    assert!(!has_event(
        &events,
        |e| matches!(e, TaskEvent::TaskStarted { task_name, .. } if task_name == "dep"),
    ));
}

#[tokio::test]
async fn test_install_expand_then_run_includes_deps() {
    let yaml = r#"
tasks:
  dep:
    commands:
      - run:
          commands: "echo dep_ran"
  leaf:
    depends_on: ["dep"]
    commands:
      - run:
          commands: "echo leaf_ran"
"#;
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.yaml");
    fs::write(&config_path, yaml).unwrap();
    let mut config = config::load_config(config_path.to_str().unwrap()).unwrap();
    config.temp_dir = dir.path().join(".ms_temp").to_string_lossy().to_string();

    let expanded =
        config::selection::expand_for_mode(&config, &["leaf".to_string()], Mode::Install, false)
            .unwrap();

    let (events, mut rx) = machine_setup::engine::sink::ChannelSink::channel();
    let runner =
        TaskRunner::new(config, Mode::Install, events).with_config_dir(dir.path().to_path_buf());
    let _ = runner.run_tasks(&expanded, true).await;
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let mut events = Vec::new();
    while let Ok(event) = rx.try_recv() {
        events.push(event);
    }

    assert!(task_completed(&events, "dep"));
    assert!(task_completed(&events, "leaf"));
    let dep_done = events
        .iter()
        .position(|e| matches!(e, TaskEvent::TaskCompleted { task_name } if task_name == "dep"))
        .unwrap();
    let leaf_start = events
        .iter()
        .position(|e| matches!(e, TaskEvent::TaskStarted { task_name, .. } if task_name == "leaf"))
        .unwrap();
    assert!(dep_done < leaf_start);
}

#[tokio::test]
async fn test_uninstall_runs_dependent_before_dependency() {
    let events = run_config(
        r#"
tasks:
  second:
    depends_on: ["first"]
    commands:
      - run:
          uninstall: "echo second_uninstall"
  first:
    commands:
      - run:
          uninstall: "echo first_uninstall"
"#,
        Mode::Uninstall,
    )
    .await;

    assert!(task_completed(&events, "first"));
    assert!(task_completed(&events, "second"));

    let second_done = events
        .iter()
        .position(|e| matches!(e, TaskEvent::TaskCompleted { task_name } if task_name == "second"))
        .unwrap();
    let first_start = events
        .iter()
        .position(|e| matches!(e, TaskEvent::TaskStarted { task_name, .. } if task_name == "first"))
        .unwrap();
    assert!(second_done < first_start);
}

#[tokio::test]
async fn test_uninstall_with_deps_list_runs_leaf_before_dep() {
    let events = run_named_tasks(
        r#"
tasks:
  dep:
    commands:
      - run:
          uninstall: "echo dep_uninstall"
  leaf:
    depends_on: ["dep"]
    commands:
      - run:
          uninstall: "echo leaf_uninstall"
"#,
        Mode::Uninstall,
        &["leaf", "dep"],
    )
    .await;

    assert!(task_completed(&events, "leaf"));
    assert!(task_completed(&events, "dep"));
    let leaf_done = events
        .iter()
        .position(|e| matches!(e, TaskEvent::TaskCompleted { task_name } if task_name == "leaf"))
        .unwrap();
    let dep_start = events
        .iter()
        .position(|e| matches!(e, TaskEvent::TaskStarted { task_name, .. } if task_name == "dep"))
        .unwrap();
    assert!(leaf_done < dep_start);
}

#[tokio::test]
async fn test_depends_on_cycle_detected() {
    use machine_setup::config::validate;

    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.yaml");
    fs::write(
        &config_path,
        r#"
tasks:
  a:
    depends_on: ["b"]
    commands:
      - run:
          commands: "echo a"
  b:
    depends_on: ["a"]
    commands:
      - run:
          commands: "echo b"
"#,
    )
    .unwrap();

    let config = config::load_config(config_path.to_str().unwrap()).unwrap();
    let issues = validate::validate_config(&config, dir.path());
    assert!(issues
        .iter()
        .any(|i| i.message.contains("Cyclic dependency")));
}

#[tokio::test]
async fn test_depends_on_missing_dependency() {
    use machine_setup::config::validate;

    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.yaml");
    fs::write(
        &config_path,
        r#"
tasks:
  a:
    depends_on: ["nonexistent"]
    commands:
      - run:
          commands: "echo a"
"#,
    )
    .unwrap();

    let config = config::load_config(config_path.to_str().unwrap()).unwrap();
    let issues = validate::validate_config(&config, dir.path());
    assert!(issues
        .iter()
        .any(|i| i.message.contains("unknown task") && i.message.contains("nonexistent")));
}

// ─── Retry tests ───

#[tokio::test]
async fn test_retry_on_failure() {
    let dir = tempdir().unwrap();
    let counter_file = dir.path().join("counter");

    let config_path = dir.path().join("config.yaml");
    fs::write(
        &config_path,
        format!(
            r#"
tasks:
  retryable:
    retry: 2
    commands:
      - run:
          commands: "if [ -f '{}' ]; then echo retry_success; else touch '{}' && exit 1; fi"
"#,
            counter_file.to_string_lossy().replace('\\', "/"),
            counter_file.to_string_lossy().replace('\\', "/"),
        ),
    )
    .unwrap();

    let mut config = config::load_config(config_path.to_str().unwrap()).unwrap();
    config.temp_dir = dir.path().join(".ms_temp").to_string_lossy().to_string();

    let (events, mut rx) = machine_setup::engine::sink::ChannelSink::channel();
    let runner =
        TaskRunner::new(config, Mode::Install, events).with_config_dir(dir.path().to_path_buf());
    let _ = runner.run_all(true).await;

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let mut events = Vec::new();
    while let Ok(event) = rx.try_recv() {
        events.push(event);
    }

    // Should have a retry event and eventual success
    assert!(events
        .iter()
        .any(|e| matches!(e, TaskEvent::TaskRetry { task_name, .. } if task_name == "retryable")));
    assert!(task_completed(&events, "retryable"));
    assert!(find_output(&events, "retryable", "retry_success"));
}

#[tokio::test]
async fn test_retry_exhausted_fails() {
    let events = run_config(
        r#"
tasks:
  always_fails:
    retry: 1
    commands:
      - run:
          commands: "exit 1"
"#,
        Mode::Install,
    )
    .await;

    // Should have a retry event but still fail
    assert!(events
        .iter()
        .any(|e| matches!(e, TaskEvent::TaskRetry { .. })));
    assert!(task_failed(&events, "always_fails"));
}

#[tokio::test]
async fn test_no_retry_by_default() {
    let events = run_config(
        r#"
tasks:
  no_retry:
    commands:
      - run:
          commands: "exit 1"
"#,
        Mode::Install,
    )
    .await;

    // Should NOT have any retry events
    assert!(!events
        .iter()
        .any(|e| matches!(e, TaskEvent::TaskRetry { .. })));
    assert!(task_failed(&events, "no_retry"));
}

// ─── Copy/symlink lifecycle tests (install → uninstall) ───

/// Run a config at `config_path` once in the given mode, against `base_dir`.
async fn run_at(config_path: &std::path::Path, base_dir: &std::path::Path, mode: Mode) {
    let config = config::load_config(config_path.to_str().unwrap()).unwrap();
    let (events, _rx) = machine_setup::engine::sink::ChannelSink::channel();
    let runner = TaskRunner::new(config, mode, events).with_config_dir(base_dir.to_path_buf());
    let _ = runner.run_all(true).await;
}

#[tokio::test]
async fn test_copy_uninstall_removes_copied_files() {
    let dir = tempdir().unwrap();
    let src_dir = dir.path().join("source");
    let target_dir = dir.path().join("target");
    fs::create_dir_all(src_dir.join("nested")).unwrap();
    fs::write(src_dir.join("a.txt"), "a").unwrap();
    fs::write(src_dir.join("nested/b.txt"), "b").unwrap();

    let config_path = dir.path().join("config.yaml");
    fs::write(
        &config_path,
        format!(
            r#"
tasks:
  copy_task:
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

    run_at(&config_path, dir.path(), Mode::Install).await;
    assert!(target_dir.join("a.txt").exists());
    assert!(target_dir.join("nested/b.txt").exists());

    run_at(&config_path, dir.path(), Mode::Uninstall).await;
    // Copied files are removed; the source is untouched.
    assert!(!target_dir.join("a.txt").exists());
    assert!(!target_dir.join("nested/b.txt").exists());
    assert!(src_dir.join("a.txt").exists());
}

#[tokio::test]
async fn test_symlink_uninstall_removes_link_keeps_source() {
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
  link_task:
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

    run_at(&config_path, dir.path(), Mode::Install).await;
    let link = target_dir.join("dotfile");
    assert!(link.symlink_metadata().is_ok());

    run_at(&config_path, dir.path(), Mode::Uninstall).await;
    assert!(link.symlink_metadata().is_err());
    // Source file survives.
    assert!(src_dir.join("dotfile").exists());
}

#[tokio::test]
async fn test_symlink_force_overwrites_existing_file() {
    let dir = tempdir().unwrap();
    let src_dir = dir.path().join("source");
    let target_dir = dir.path().join("target");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&target_dir).unwrap();
    fs::write(src_dir.join("dotfile"), "from-source").unwrap();
    // A real file already sits where the symlink should go.
    fs::write(target_dir.join("dotfile"), "pre-existing").unwrap();

    let config_path = dir.path().join("config.yaml");
    fs::write(
        &config_path,
        format!(
            r#"
tasks:
  link_task:
    commands:
      - symlink:
          src: "{}"
          target: "{}"
          force: true
"#,
            src_dir.to_string_lossy().replace('\\', "/"),
            target_dir.to_string_lossy().replace('\\', "/"),
        ),
    )
    .unwrap();

    run_at(&config_path, dir.path(), Mode::Install).await;

    let link = target_dir.join("dotfile");
    let meta = link.symlink_metadata().unwrap();
    assert!(
        meta.file_type().is_symlink(),
        "force should replace the file with a symlink"
    );
    // Reading through the link yields the source content.
    assert_eq!(fs::read_to_string(&link).unwrap(), "from-source");
}

#[tokio::test]
async fn test_symlink_without_force_skips_existing_file() {
    let dir = tempdir().unwrap();
    let src_dir = dir.path().join("source");
    let target_dir = dir.path().join("target");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&target_dir).unwrap();
    fs::write(src_dir.join("dotfile"), "from-source").unwrap();
    fs::write(target_dir.join("dotfile"), "pre-existing").unwrap();

    let config_path = dir.path().join("config.yaml");
    fs::write(
        &config_path,
        format!(
            r#"
tasks:
  link_task:
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

    run_at(&config_path, dir.path(), Mode::Install).await;

    // Without force, the pre-existing real file is left in place.
    let link = target_dir.join("dotfile");
    assert!(!link.symlink_metadata().unwrap().file_type().is_symlink());
    assert_eq!(fs::read_to_string(&link).unwrap(), "pre-existing");
}

#[tokio::test]
async fn test_parallel_respects_dependency_layers() {
    // parallel: true must still honor depends_on: the dependency completes
    // before the dependent starts, exercising the graph's layering.
    let events = run_config(
        r#"
parallel: true
tasks:
  dependent:
    depends_on: ["base"]
    commands:
      - run:
          commands: "echo dependent_task"
  base:
    commands:
      - run:
          commands: "echo base_task"
"#,
        Mode::Install,
    )
    .await;

    assert!(task_completed(&events, "base"));
    assert!(task_completed(&events, "dependent"));

    let base_done = events
        .iter()
        .position(|e| matches!(e, TaskEvent::TaskCompleted { task_name } if task_name == "base"))
        .unwrap();
    let dependent_start = events
        .iter()
        .position(
            |e| matches!(e, TaskEvent::TaskStarted { task_name, .. } if task_name == "dependent"),
        )
        .unwrap();
    assert!(base_done < dependent_start);
}

#[tokio::test]
async fn test_copy_single_file_into_directory_target() {
    // A single source file with an existing directory target lands inside the
    // directory under its own name (the dest-resolution rule).
    let dir = tempdir().unwrap();
    let src_file = dir.path().join("config.toml");
    fs::write(&src_file, "x = 1").unwrap();
    let target_dir = dir.path().join("dest_dir");
    fs::create_dir_all(&target_dir).unwrap();

    let config_path = dir.path().join("config.yaml");
    fs::write(
        &config_path,
        format!(
            r#"
tasks:
  copy_one:
    commands:
      - copy:
          src: "{}"
          target: "{}"
"#,
            src_file.to_string_lossy().replace('\\', "/"),
            target_dir.to_string_lossy().replace('\\', "/"),
        ),
    )
    .unwrap();

    run_at(&config_path, dir.path(), Mode::Install).await;
    assert!(target_dir.join("config.toml").exists());
    assert_eq!(
        fs::read_to_string(target_dir.join("config.toml")).unwrap(),
        "x = 1"
    );
}
