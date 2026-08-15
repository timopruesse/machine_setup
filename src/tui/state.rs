use std::time::{Duration, Instant};

use crate::engine::mode::Mode;

/// Max log lines retained per task (oldest dropped).
pub const LOG_CAP: usize = 2000;

/// Max lines retained in the parallel merge buffer (oldest dropped).
pub const MERGE_CAP: usize = 2000;

/// Number of distinct accent colors for running / merge tags.
pub const TASK_PALETTE_LEN: usize = 8;

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

    pub fn is_running(&self) -> bool {
        matches!(self, TaskStatus::Running)
    }
}

/// One line in the ephemeral parallel merge stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeLine {
    pub task_name: String,
    pub color_idx: usize,
    pub text: String,
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
    /// When the task last entered Running.
    pub started_at: Option<Instant>,
    /// Frozen duration once the task reaches a terminal status.
    pub duration: Option<Duration>,
    /// Stable accent into [`TASK_PALETTE_LEN`] for this run.
    pub color_idx: Option<usize>,
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
            started_at: None,
            duration: None,
            color_idx: None,
        }
    }

    /// Append a log line, enforcing [`LOG_CAP`].
    pub fn push_log(&mut self, line: String) {
        let line = crate::tui::format::strip_ansi(&line);
        self.log_lines.push(line);
        if self.log_lines.len() > LOG_CAP {
            let excess = self.log_lines.len() - LOG_CAP;
            self.log_lines.drain(0..excess);
        }
    }

    /// Mark the task as running and stamp a fresh start time.
    pub fn mark_running(&mut self) {
        self.status = TaskStatus::Running;
        self.started_at = Some(Instant::now());
        self.duration = None;
    }

    /// Freeze elapsed time if the task had started.
    pub fn freeze_duration(&mut self) {
        if self.duration.is_none() {
            if let Some(started) = self.started_at {
                self.duration = Some(started.elapsed());
            }
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
    /// Auto-follow: soft-track a running task when selection is idle
    pub auto_select_running: bool,
    pub search_mode: bool,
    pub search_query: String,
    /// Indices into `tasks` that match the current filter
    pub filtered_indices: Vec<usize>,
    pub tick: u64,
    /// When the TUI run started.
    pub run_started: Instant,
    /// Frozen run duration once all tasks are done.
    pub run_elapsed: Option<Duration>,
    /// Live multiplex while ≥2 tasks are running.
    pub merge_lines: Vec<MergeLine>,
    /// Task indices that failed during the current merge burst (order preserved).
    pub merge_failed: Vec<usize>,
    /// Next palette slot to hand out.
    pub next_color: usize,
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
            run_started: Instant::now(),
            run_elapsed: None,
            merge_lines: Vec::new(),
            merge_failed: Vec::new(),
            next_color: 0,
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

    pub fn running_count(&self) -> usize {
        self.tasks.iter().filter(|t| t.status.is_running()).count()
    }

    pub fn in_merge_mode(&self) -> bool {
        self.running_count() >= 2
    }

    /// True when a filter query is retained (search mode or non-empty query).
    pub fn filter_active(&self) -> bool {
        self.search_mode || !self.search_query.is_empty()
    }

    pub fn spinner_frame(&self) -> &'static str {
        const FRAMES: [&str; 4] = ["|", "/", "-", "\\"];
        FRAMES[(self.tick % 4) as usize]
    }

    /// Freeze the run clock if not already frozen.
    pub fn freeze_run_elapsed(&mut self) {
        if self.run_elapsed.is_none() {
            self.run_elapsed = Some(self.run_started.elapsed());
        }
    }

    /// Assign a stable palette index for `task_name` if needed.
    pub fn ensure_task_color(&mut self, task_name: &str) -> usize {
        if let Some(task) = self.tasks.iter().find(|t| t.name == task_name) {
            if let Some(idx) = task.color_idx {
                return idx;
            }
        }
        let idx = self.next_color % TASK_PALETTE_LEN;
        self.next_color = self.next_color.wrapping_add(1);
        if let Some(task) = self.tasks.iter_mut().find(|t| t.name == task_name) {
            task.color_idx = Some(idx);
        }
        idx
    }

    /// Append a merge line, enforcing [`MERGE_CAP`].
    pub fn push_merge(&mut self, line: MergeLine) {
        let line = MergeLine {
            task_name: line.task_name,
            color_idx: line.color_idx,
            text: crate::tui::format::strip_ansi(&line.text),
        };
        self.merge_lines.push(line);
        if self.merge_lines.len() > MERGE_CAP {
            let excess = self.merge_lines.len() - MERGE_CAP;
            self.merge_lines.drain(0..excess);
        }
    }
}
