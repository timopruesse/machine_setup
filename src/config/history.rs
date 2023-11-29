use crate::task_runner::TaskRunnerMode;
use chrono::Utc;
use ergo_fs::{IoWrite, Path, Read};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub struct TaskEntry {
    installed_at: Option<String>,
    updated_at: Option<String>,
    uninstalled_at: Option<String>,
}

pub fn get_history_file(temp_dir: &str) -> Result<File, std::io::Error> {
    let path = Path::new(temp_dir);

    if !path.exists() {
        std::fs::create_dir_all(temp_dir)?;
    }

    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(path.join("history.json"))
}

pub fn get_task_entry(temp_dir: &str, task_name: &str) -> Result<TaskEntry, String> {
    let file = get_history_file(temp_dir);
    if let Err(file_err) = file {
        return Err(file_err.to_string());
    }
    let mut file = file.unwrap();

    let mut file_contents = String::new();
    if let Err(file_err) = file.read_to_string(&mut file_contents) {
        return Err(format!("[{}] {}", task_name, file_err));
    }

    let mut entries: BTreeMap<String, TaskEntry> =
        serde_json::from_str(&file_contents).unwrap_or_else(|_| BTreeMap::new());

    let entry: &TaskEntry = entries.entry(task_name.to_owned()).or_insert(TaskEntry {
        installed_at: None,
        updated_at: None,
        uninstalled_at: None,
    });

    Ok(entry.to_owned())
}

pub fn is_logged(temp_dir: &str, mode: &TaskRunnerMode, task_name: &str) -> bool {
    let entry = get_task_entry(temp_dir, task_name);

    if entry.is_err() {
        return false;
    }

    let entry = entry.unwrap();

    match mode {
        TaskRunnerMode::Install => entry.installed_at.is_some(),
        TaskRunnerMode::Update => entry.updated_at.is_some(),
        TaskRunnerMode::Uninstall => entry.uninstalled_at.is_some(),
    }
}

pub fn clear_entry(temp_dir: &str, mode: &TaskRunnerMode, task_name: &str) -> Result<(), String> {
    let file = get_history_file(temp_dir);
    if let Err(file_err) = file {
        return Err(file_err.to_string());
    }
    let mut file = file.unwrap();

    let mut file_contents = String::new();
    if let Err(file_err) = file.read_to_string(&mut file_contents) {
        return Err(format!("[{}] {}", task_name, file_err));
    }

    let mut entries: BTreeMap<String, TaskEntry> =
        serde_json::from_str(&file_contents).unwrap_or_else(|_| BTreeMap::new());

    let task_entry = entries.entry(task_name.to_owned()).or_insert(TaskEntry {
        installed_at: None,
        updated_at: None,
        uninstalled_at: None,
    });

    match mode {
        TaskRunnerMode::Install => task_entry.installed_at = None,
        TaskRunnerMode::Update => task_entry.updated_at = None,
        TaskRunnerMode::Uninstall => task_entry.uninstalled_at = None,
    }

    file.set_len(0).unwrap_or_default();
    if let Err(write_err) = file.write_all(serde_json::to_string(&entries).unwrap().as_bytes()) {
        return Err(format!("{}: {}", task_name, write_err));
    }

    Ok(())
}

pub fn update_entry(temp_dir: &str, mode: &TaskRunnerMode, task_name: &str) -> Result<(), String> {
    let file = get_history_file(temp_dir);

    if let Err(file_err) = file {
        return Err(file_err.to_string());
    }
    let mut file = file.unwrap();

    let timestamp = Utc::now().to_rfc3339();

    let mut file_contents = String::new();

    if let Err(file_err) = file.read_to_string(&mut file_contents) {
        return Err(format!("[{}] {}", task_name, file_err));
    }

    let mut entries: BTreeMap<String, TaskEntry> =
        serde_json::from_str(&file_contents).unwrap_or_else(|_| BTreeMap::new());

    let task_entry = entries.entry(task_name.to_owned()).or_insert(TaskEntry {
        installed_at: None,
        updated_at: None,
        uninstalled_at: None,
    });

    match mode {
        TaskRunnerMode::Install => {
            task_entry.installed_at = Some(timestamp);
            task_entry.uninstalled_at = None;
        }
        TaskRunnerMode::Update => task_entry.updated_at = Some(timestamp),
        TaskRunnerMode::Uninstall => {
            task_entry.uninstalled_at = Some(timestamp);
            task_entry.installed_at = None;
        }
    }

    if let Err(trunc_err) = file.set_len(0) {
        return Err(format!("{}: {}", task_name, trunc_err));
    }

    if let Err(write_err) = file.write_all(serde_json::to_string(&entries).unwrap().as_bytes()) {
        return Err(format!("{}: {}", task_name, write_err));
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use chrono::DateTime;
    use std::env::temp_dir;
    use std::fs::read_to_string;

    use super::*;

    #[test]
    fn it_clears_task_entry() {
        let temp_dir = temp_dir();
        let file_path = temp_dir.join("history.json");

        std::fs::write(
            file_path.to_str().unwrap(),
            r#"{
            "task1": {
                "installed_at": "2022-12-31T23:59:59+00:00",
                "updated_at": "2022-12-31T23:59:59+00:00",
                "uninstalled_at": "2022-12-31T23:59:59+00:00"
            }
        }"#
            .as_bytes(),
        )
        .unwrap();

        let result = clear_entry(
            temp_dir.to_str().unwrap(),
            &TaskRunnerMode::Install,
            "task1",
        );
        assert!(result.is_ok());
        let result = clear_entry(
            temp_dir.to_str().unwrap(),
            &TaskRunnerMode::Uninstall,
            "task1",
        );
        assert!(result.is_ok());
        let result = clear_entry(temp_dir.to_str().unwrap(), &TaskRunnerMode::Update, "task1");
        assert!(result.is_ok());

        let file_contents = read_to_string(&file_path).unwrap();
        let entries: BTreeMap<String, TaskEntry> = serde_json::from_str(&file_contents).unwrap();
        let entry = &entries["task1"];

        assert_eq!(
            entry,
            &TaskEntry {
                installed_at: None,
                uninstalled_at: None,
                updated_at: None
            }
        );
    }

    #[test]
    fn it_gets_empty_entry_if_file_doesnt_exist() {
        let temp_dir = temp_dir();
        let result = get_task_entry(temp_dir.join("nope").to_str().unwrap(), "task");
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            TaskEntry {
                updated_at: None,
                uninstalled_at: None,
                installed_at: None
            }
        );
    }

    #[test]
    fn it_gets_task_entry() {
        let temp_dir = temp_dir();
        let file_path = temp_dir.join("history.json");

        std::fs::write(
            file_path.to_str().unwrap(),
            r#"{
            "task1": {
                "installed_at": "2022-12-31T23:59:59+00:00",
                "updated_at": null,
                "uninstalled_at": null
            },
            "task2": {
                "installed_at": "2023-01-31T23:59:59+00:00",
                "updated_at": null,
                "uninstalled_at": null
            }
        }"#
            .as_bytes(),
        )
        .unwrap();

        let result = get_task_entry(temp_dir.to_str().unwrap(), "task2");
        assert!(result.is_ok());

        assert_eq!(
            result.unwrap().installed_at.unwrap(),
            "2023-01-31T23:59:59+00:00"
        );
    }

    #[test]
    fn it_gets_installation_status_correctly() {
        let temp_dir = temp_dir();
        let file_path = temp_dir.join("history.json");

        std::fs::write(
            file_path.to_str().unwrap(),
            r#"{
            "task1": {
                "installed_at": "2022-12-31T23:59:59+00:00",
                "updated_at": null,
                "uninstalled_at": null
            },
            "task2": {
                "installed_at": null,
                "updated_at": null,
                "uninstalled_at": null
            }
        }"#
            .as_bytes(),
        )
        .unwrap();

        let logged = is_logged(
            temp_dir.to_str().unwrap(),
            &TaskRunnerMode::Install,
            "task1",
        );
        assert!(logged);

        let logged = is_logged(
            temp_dir.to_str().unwrap(),
            &TaskRunnerMode::Install,
            "task2",
        );
        assert!(!logged);
    }

    #[test]
    fn it_updates_entry_in_existing_file() {
        let temp_dir = temp_dir();
        let file_path = temp_dir.join("history.json");

        std::fs::write(
            file_path.to_str().unwrap(),
            r#"{
            "task1": {
                "installed_at": "2022-12-31T23:59:59+00:00",
                "updated_at": null,
                "uninstalled_at": null
            },
            "task2": {
                "installed_at": null,
                "updated_at": null,
                "uninstalled_at": "2022-12-31T23:59:59+00:00"
            }
        }"#
            .as_bytes(),
        )
        .unwrap();

        let result = update_entry(
            temp_dir.to_str().unwrap(),
            &TaskRunnerMode::Install,
            "task2",
        );
        assert!(result.is_ok());

        let file_contents = read_to_string(&file_path).unwrap();
        let entries: BTreeMap<String, TaskEntry> = serde_json::from_str(&file_contents).unwrap();
        let entry = &entries["task2"];

        assert!(DateTime::parse_from_rfc3339(entry.installed_at.as_ref().unwrap()).is_ok());
        assert!(entry.uninstalled_at.is_none());

        let result = update_entry(
            temp_dir.to_str().unwrap(),
            &TaskRunnerMode::Uninstall,
            "task1",
        );
        assert!(result.is_ok());

        let file_contents = read_to_string(&file_path).unwrap();
        let entries: BTreeMap<String, TaskEntry> = serde_json::from_str(&file_contents).unwrap();
        let entry = &entries["task1"];

        assert!(DateTime::parse_from_rfc3339(entry.uninstalled_at.as_ref().unwrap()).is_ok());
        assert!(entry.installed_at.is_none());
    }

    #[test]
    fn it_inserts_entry_into_existing_file() {
        let temp_dir = temp_dir();
        let file_path = temp_dir.join("history.json");

        std::fs::write(
            file_path.to_str().unwrap(),
            r#"{
            "task1": {
                "installed_at": null,
                "updated_at": null,
                "uninstalled_at": null
            },
        }"#
            .as_bytes(),
        )
        .unwrap();

        let result = update_entry(
            temp_dir.to_str().unwrap(),
            &TaskRunnerMode::Install,
            "task2",
        );
        assert!(result.is_ok());

        let file_contents = read_to_string(file_path).unwrap();
        let entries: BTreeMap<String, TaskEntry> = serde_json::from_str(&file_contents).unwrap();
        let entry = &entries["task2"];

        assert!(DateTime::parse_from_rfc3339(entry.installed_at.as_ref().unwrap()).is_ok());
        assert!(entry.uninstalled_at.is_none());
    }

    #[test]
    fn it_inserts_entry_into_fresh_file() {
        let temp_dir = temp_dir();

        let result = update_entry(
            temp_dir.to_str().unwrap(),
            &TaskRunnerMode::Install,
            "task1",
        );
        assert!(result.is_ok());

        let file_path = temp_dir.join("history.json");
        let file_contents = read_to_string(file_path).unwrap();

        let entries: BTreeMap<String, TaskEntry> = serde_json::from_str(&file_contents).unwrap();
        let entry = &entries["task1"];

        assert!(DateTime::parse_from_rfc3339(entry.installed_at.as_ref().unwrap()).is_ok());
        assert!(entry.uninstalled_at.is_none());
    }

    #[test]
    fn it_creates_history_file_if_needed() {
        let temp_dir = temp_dir();

        let result = update_entry(
            temp_dir.to_str().unwrap(),
            &TaskRunnerMode::Install,
            "task1",
        );
        assert!(result.is_ok());

        let file_path = temp_dir.join("history.json");
        let file_contents = read_to_string(file_path).unwrap();

        let entries: BTreeMap<String, TaskEntry> = serde_json::from_str(&file_contents).unwrap();
        let entry = &entries["task1"];

        assert!(DateTime::parse_from_rfc3339(entry.installed_at.as_ref().unwrap()).is_ok());
        assert!(entry.uninstalled_at.is_none());
    }
}
