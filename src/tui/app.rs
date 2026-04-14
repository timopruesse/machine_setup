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
    /// Nesting depth (0 = top-level, 1+ = sub-config)
    pub depth: usize,
}

impl TaskState {
    pub fn new(name: String) -> Self {
        Self {
            name,
            status: TaskStatus::Pending,
            log_lines: Vec::new(),
            command_count: 0,
            current_command: None,
            depth: 0,
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
    /// Whether the search input is active
    pub search_mode: bool,
    /// Current search query
    pub search_query: String,
    /// Indices into `tasks` that match the current filter
    pub filtered_indices: Vec<usize>,
}

impl App {
    pub fn new(task_names: Vec<String>, mode: Command) -> Self {
        let filtered_indices: Vec<usize> = (0..task_names.len()).collect();
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
            search_mode: false,
            search_query: String::new(),
            filtered_indices,
        }
    }

    /// Process a task event and update state.
    /// Tasks from sub-configs are added dynamically if not already known.
    pub fn handle_event(&mut self, event: TaskEvent) {
        match event {
            TaskEvent::TaskStarted {
                task_name,
                command_count,
                depth,
            } => {
                let task = self.find_or_create_task(&task_name);
                task.status = TaskStatus::Running;
                task.command_count = command_count;
                task.depth = depth;
                task.log_lines
                    .push(format!("Starting ({command_count} commands)..."));
                if self.auto_select {
                    self.select_task(&task_name);
                }
            }
            TaskEvent::TaskSkipped { task_name, reason } => {
                let task = self.find_or_create_task(&task_name);
                task.status = TaskStatus::Skipped(reason.clone());
                task.log_lines.push(format!("Skipped: {reason}"));
                self.skipped += 1;
            }
            TaskEvent::CommandStarted {
                task_name,
                command_desc,
            } => {
                let task = self.find_or_create_task(&task_name);
                task.current_command = Some(command_desc.clone());
                task.log_lines.push(format!("> {command_desc}"));
                self.auto_scroll_log();
            }
            TaskEvent::CommandOutput { task_name, line } => {
                let task = self.find_or_create_task(&task_name);
                task.log_lines.push(format!("  {line}"));
                self.auto_scroll_log();
            }
            TaskEvent::CommandCompleted {
                task_name,
                command_desc,
            } => {
                let task = self.find_or_create_task(&task_name);
                task.current_command = None;
                task.log_lines.push(format!("  [done] {command_desc}"));
            }
            TaskEvent::CommandFailed {
                task_name,
                command_desc,
                error,
            } => {
                let task = self.find_or_create_task(&task_name);
                task.current_command = None;
                task.log_lines
                    .push(format!("  [FAILED] {command_desc}: {error}"));
            }
            TaskEvent::TaskCompleted { task_name } => {
                let task = self.find_or_create_task(&task_name);
                task.status = TaskStatus::Completed;
                task.log_lines.push("Completed successfully.".to_string());
                self.succeeded += 1;
            }
            TaskEvent::TaskFailed { task_name, error } => {
                let task = self.find_or_create_task(&task_name);
                task.status = TaskStatus::Failed(error.clone());
                task.log_lines.push(format!("FAILED: {error}"));
                self.failed += 1;
            }
            TaskEvent::TaskRetry {
                task_name,
                attempt,
                max_attempts,
                error,
            } => {
                let task = self.find_or_create_task(&task_name);
                task.status = TaskStatus::Running;
                task.log_lines
                    .push(format!("  Retry {attempt}/{max_attempts}: {error}"));
                self.auto_scroll_log();
            }
            TaskEvent::AllDone { .. } => {
                self.done = true;
            }
        }
    }

    pub fn select_next(&mut self) {
        self.auto_select = false;
        if self.filtered_indices.is_empty() {
            return;
        }
        // Find the next filtered index after current selection
        if let Some(pos) = self
            .filtered_indices
            .iter()
            .position(|&i| i > self.selected)
        {
            self.selected = self.filtered_indices[pos];
        }
        self.log_scroll = 0;
    }

    pub fn select_prev(&mut self) {
        self.auto_select = false;
        if self.filtered_indices.is_empty() {
            return;
        }
        // Find the previous filtered index before current selection
        if let Some(pos) = self
            .filtered_indices
            .iter()
            .rposition(|&i| i < self.selected)
        {
            self.selected = self.filtered_indices[pos];
        }
        self.log_scroll = 0;
    }

    /// Enter search mode.
    pub fn enter_search(&mut self) {
        self.search_mode = true;
        self.search_query.clear();
        self.update_filter();
    }

    /// Exit search mode and clear the filter.
    pub fn exit_search(&mut self) {
        self.search_mode = false;
        self.search_query.clear();
        self.update_filter();
    }

    /// Confirm search: exit search mode but keep the filter active.
    pub fn confirm_search(&mut self) {
        self.search_mode = false;
    }

    /// Append a character to the search query.
    pub fn search_push_char(&mut self, c: char) {
        self.search_query.push(c);
        self.update_filter();
    }

    /// Remove the last character from the search query.
    pub fn search_pop_char(&mut self) {
        self.search_query.pop();
        self.update_filter();
    }

    /// Recalculate filtered indices based on the search query.
    fn update_filter(&mut self) {
        let query = self.search_query.to_lowercase();
        self.filtered_indices = self
            .tasks
            .iter()
            .enumerate()
            .filter(|(_, task)| query.is_empty() || task.name.to_lowercase().contains(&query))
            .map(|(i, _)| i)
            .collect();

        // If selected is no longer in filtered set, jump to nearest match
        if !self.filtered_indices.contains(&self.selected) {
            if let Some(&first) = self.filtered_indices.first() {
                self.selected = first;
                self.log_scroll = 0;
            }
        }
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

    /// Find a task by name, or create it if it doesn't exist (e.g. sub-config tasks).
    fn find_or_create_task(&mut self, name: &str) -> &mut TaskState {
        if !self.tasks.iter().any(|t| t.name == name) {
            let idx = self.tasks.len();
            self.tasks.push(TaskState::new(name.to_string()));
            // Add to filtered indices if it matches the current filter
            let query = self.search_query.to_lowercase();
            if query.is_empty() || name.to_lowercase().contains(&query) {
                self.filtered_indices.push(idx);
            }
        }
        self.tasks.iter_mut().find(|t| t.name == name).unwrap()
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
