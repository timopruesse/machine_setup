use crate::cli::Command;
use crate::engine::event::TaskEvent;

/// Status of a task in the TUI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed(String),
    Skipped(String),
}

impl TaskStatus {
    #[allow(dead_code)]
    pub fn symbol(&self) -> &str {
        match self {
            TaskStatus::Pending => "  ",
            TaskStatus::Running => ">>",
            TaskStatus::Completed => "OK",
            TaskStatus::Failed(_) => "XX",
            TaskStatus::Skipped(_) => "--",
        }
    }

    #[allow(dead_code)]
    pub fn is_done(&self) -> bool {
        matches!(
            self,
            TaskStatus::Completed | TaskStatus::Failed(_) | TaskStatus::Skipped(_)
        )
    }
}

/// State for a single task in the TUI.
#[derive(Debug, Clone)]
pub struct TaskState {
    pub name: String,
    pub status: TaskStatus,
    pub log_lines: Vec<String>,
    pub command_count: usize,
    pub current_command: Option<String>,
}

impl TaskState {
    pub fn new(name: String) -> Self {
        Self {
            name,
            status: TaskStatus::Pending,
            log_lines: Vec::new(),
            command_count: 0,
            current_command: None,
        }
    }
}

/// The TUI application state.
pub struct App {
    pub tasks: Vec<TaskState>,
    pub selected: usize,
    pub mode: Command,
    pub log_scroll: usize,
    pub done: bool,
    pub succeeded: usize,
    pub failed: usize,
    pub skipped: usize,
    /// Auto-follow: track the first running task
    pub auto_select: bool,
}

impl App {
    pub fn new(task_names: Vec<String>, mode: Command) -> Self {
        let tasks = task_names.into_iter().map(TaskState::new).collect();
        Self {
            tasks,
            selected: 0,
            mode,
            log_scroll: 0,
            done: false,
            succeeded: 0,
            failed: 0,
            skipped: 0,
            auto_select: true,
        }
    }

    /// Process a task event and update state.
    pub fn handle_event(&mut self, event: TaskEvent) {
        match event {
            TaskEvent::TaskStarted {
                task_name,
                command_count,
            } => {
                if let Some(task) = self.find_task_mut(&task_name) {
                    task.status = TaskStatus::Running;
                    task.command_count = command_count;
                    task.log_lines
                        .push(format!("Starting ({command_count} commands)..."));
                }
                if self.auto_select {
                    self.select_task(&task_name);
                }
            }
            TaskEvent::TaskSkipped { task_name, reason } => {
                if let Some(task) = self.find_task_mut(&task_name) {
                    task.status = TaskStatus::Skipped(reason.clone());
                    task.log_lines.push(format!("Skipped: {reason}"));
                }
                self.skipped += 1;
            }
            TaskEvent::CommandStarted {
                task_name,
                command_desc,
            } => {
                if let Some(task) = self.find_task_mut(&task_name) {
                    task.current_command = Some(command_desc.clone());
                    task.log_lines.push(format!("> {command_desc}"));
                }
                self.auto_scroll_log();
            }
            TaskEvent::CommandOutput { task_name, line } => {
                if let Some(task) = self.find_task_mut(&task_name) {
                    task.log_lines.push(format!("  {line}"));
                }
                self.auto_scroll_log();
            }
            TaskEvent::CommandCompleted {
                task_name,
                command_desc,
            } => {
                if let Some(task) = self.find_task_mut(&task_name) {
                    task.current_command = None;
                    task.log_lines.push(format!("  [done] {command_desc}"));
                }
            }
            TaskEvent::CommandFailed {
                task_name,
                command_desc,
                error,
            } => {
                if let Some(task) = self.find_task_mut(&task_name) {
                    task.current_command = None;
                    task.log_lines
                        .push(format!("  [FAILED] {command_desc}: {error}"));
                }
            }
            TaskEvent::TaskCompleted { task_name } => {
                if let Some(task) = self.find_task_mut(&task_name) {
                    task.status = TaskStatus::Completed;
                    task.log_lines.push("Completed successfully.".to_string());
                }
                self.succeeded += 1;
            }
            TaskEvent::TaskFailed { task_name, error } => {
                if let Some(task) = self.find_task_mut(&task_name) {
                    task.status = TaskStatus::Failed(error.clone());
                    task.log_lines.push(format!("FAILED: {error}"));
                }
                self.failed += 1;
            }
            TaskEvent::AllDone { .. } => {
                self.done = true;
            }
        }
    }

    pub fn select_next(&mut self) {
        self.auto_select = false;
        if !self.tasks.is_empty() {
            self.selected = (self.selected + 1).min(self.tasks.len() - 1);
            self.log_scroll = 0;
        }
    }

    pub fn select_prev(&mut self) {
        self.auto_select = false;
        self.selected = self.selected.saturating_sub(1);
        self.log_scroll = 0;
    }

    pub fn scroll_log_down(&mut self) {
        self.log_scroll += 3;
    }

    pub fn scroll_log_up(&mut self) {
        self.log_scroll = self.log_scroll.saturating_sub(3);
    }

    pub fn scroll_log_to_top(&mut self) {
        self.log_scroll = 0;
    }

    pub fn scroll_log_to_bottom(&mut self) {
        if let Some(task) = self.tasks.get(self.selected) {
            self.log_scroll = task.log_lines.len().saturating_sub(1);
        }
    }

    pub fn selected_task(&self) -> Option<&TaskState> {
        self.tasks.get(self.selected)
    }

    pub fn total_tasks(&self) -> usize {
        self.tasks.len()
    }

    pub fn completed_tasks(&self) -> usize {
        self.succeeded + self.failed + self.skipped
    }

    fn find_task_mut(&mut self, name: &str) -> Option<&mut TaskState> {
        self.tasks.iter_mut().find(|t| t.name == name)
    }

    fn select_task(&mut self, name: &str) {
        if let Some(idx) = self.tasks.iter().position(|t| t.name == name) {
            self.selected = idx;
            self.log_scroll = 0;
        }
    }

    fn auto_scroll_log(&mut self) {
        // Keep log scrolled to bottom for the selected task
        if let Some(task) = self.tasks.get(self.selected) {
            let line_count = task.log_lines.len();
            if line_count > 0 {
                self.log_scroll = line_count.saturating_sub(1);
            }
        }
    }
}
