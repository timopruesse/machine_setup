use crate::engine::mode::Mode;

/// Max log lines retained per task (oldest dropped).
pub const LOG_CAP: usize = 2000;

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
    pub fn is_failed(&self) -> bool {
        matches!(self, TaskStatus::Failed(_))
    }

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

    /// Append a log line, enforcing [`LOG_CAP`].
    pub fn push_log(&mut self, line: String) {
        self.log_lines.push(line);
        if self.log_lines.len() > LOG_CAP {
            let excess = self.log_lines.len() - LOG_CAP;
            self.log_lines.drain(0..excess);
        }
    }
}

/// The TUI application state (pure data; mutated only via `reduce`).
#[derive(Debug, Clone)]
pub struct UiState {
    pub tasks: Vec<TaskState>,
    pub selected: usize,
    pub mode: Mode,
    pub log_scroll: usize,
    pub log_follow: bool,
    pub done: bool,
    pub succeeded: usize,
    pub failed: usize,
    pub skipped: usize,
    /// Auto-follow: track the first running task
    pub auto_select_running: bool,
    pub search_mode: bool,
    pub search_query: String,
    /// Indices into `tasks` that match the current filter
    pub filtered_indices: Vec<usize>,
    pub tick: u64,
}

impl UiState {
    pub fn new(task_names: Vec<String>, mode: Mode) -> Self {
        let filtered_indices: Vec<usize> = (0..task_names.len()).collect();
        let tasks = task_names.into_iter().map(TaskState::new).collect();
        Self {
            tasks,
            selected: 0,
            mode,
            log_scroll: 0,
            log_follow: true,
            done: false,
            succeeded: 0,
            failed: 0,
            skipped: 0,
            auto_select_running: true,
            search_mode: false,
            search_query: String::new(),
            filtered_indices,
            tick: 0,
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

    /// True when a filter query is retained (search mode or non-empty query).
    pub fn filter_active(&self) -> bool {
        self.search_mode || !self.search_query.is_empty()
    }

    pub fn spinner_frame(&self) -> &'static str {
        const FRAMES: [&str; 4] = ["|", "/", "-", "\\"];
        FRAMES[(self.tick % 4) as usize]
    }
}
