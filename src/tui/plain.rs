use tokio::sync::mpsc;

use crate::engine::event::TaskEvent;

/// Plain text event consumer for --no-tui / CI environments.
pub async fn run(mut event_rx: mpsc::UnboundedReceiver<TaskEvent>) {
    while let Some(event) = event_rx.recv().await {
        match &event {
            TaskEvent::TaskStarted {
                task_name,
                command_count,
                depth,
            } => {
                let indent = "  ".repeat(*depth);
                println!("{indent}>> Starting: {task_name} ({command_count} commands)");
            }
            TaskEvent::TaskSkipped { task_name, reason } => {
                println!("-- Skipped: {task_name} ({reason})");
            }
            TaskEvent::CommandStarted {
                task_name,
                command_desc,
            } => {
                println!("   [{task_name}] > {command_desc}");
            }
            TaskEvent::CommandOutput { task_name, line } => {
                println!("   [{task_name}]   {line}");
            }
            TaskEvent::CommandCompleted {
                task_name,
                command_desc,
            } => {
                println!("   [{task_name}]   [done] {command_desc}");
            }
            TaskEvent::CommandFailed {
                task_name,
                command_desc,
                error,
            } => {
                eprintln!("   [{task_name}]   [FAILED] {command_desc}: {error}");
            }
            TaskEvent::TaskCompleted { task_name } => {
                println!("OK {task_name}");
            }
            TaskEvent::TaskFailed { task_name, error } => {
                eprintln!("XX {task_name}: {error}");
            }
            TaskEvent::TaskRetry {
                task_name,
                attempt,
                max_attempts,
                error,
            } => {
                println!("   [{task_name}]   Retry {attempt}/{max_attempts}: {error}");
            }
            TaskEvent::AllDone {
                succeeded,
                failed,
                skipped,
            } => {
                println!("\n== Done: {succeeded} succeeded, {failed} failed, {skipped} skipped ==");
            }
        }
    }
}
