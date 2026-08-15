//! OS-timer auto-update schedules (launchd / systemd user) + shell notices.
//!
//! Prefer the names **schedule** / **auto_update** / **OS timer** — not
//! “scheduler” (reserved elsewhere in CONTEXT.md for Task graph / concurrency).

pub mod apply;
pub mod group;
pub mod hook;
pub mod key;
pub mod manifest;
pub mod notices;
pub mod platform;
pub mod run;
pub mod status;

pub use key::ScheduleKey;
