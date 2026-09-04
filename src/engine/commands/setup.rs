use async_trait::async_trait;
use std::sync::Arc;

use crate::config::types::MachineSetupArgs;
use crate::engine::context::CommandContext;
use crate::error::Result;
use crate::utils::path::expand_path;

use super::CommandExecutor;

pub struct SetupCommand {
    args: MachineSetupArgs,
}

impl SetupCommand {
    pub fn new(args: MachineSetupArgs) -> Self {
        Self { args }
    }
}

#[async_trait]
impl CommandExecutor for SetupCommand {
    async fn execute(&self, ctx: &CommandContext) -> Result<()> {
        run_sub_config(&self.args, ctx).await
    }

    fn description(&self) -> String {
        self.args.to_string()
    }

    fn occupies_concurrency_slot(&self) -> bool {
        // Nested Runner shares the parent gate; holding a permit here would
        // deadlock when nested leaf commands try to acquire (ADR-0003).
        false
    }
}

async fn run_sub_config(args: &MachineSetupArgs, ctx: &CommandContext) -> Result<()> {
    let is_url = crate::config::is_url(&args.config);

    // For URLs we pass the string through to load_config; for local paths
    // we resolve via expand_path against the parent's config_dir.
    let config_str: std::borrow::Cow<'_, str> = if is_url {
        std::borrow::Cow::Borrowed(&args.config)
    } else {
        let path = expand_path(&args.config, Some(&ctx.config_dir));
        std::borrow::Cow::Owned(path.to_string_lossy().into_owned())
    };

    ctx.log_info(format!("Loading sub-config: {config_str}"));

    let config = crate::config::load_config(&config_str)?;

    let run_set = match (&args.task, args.with_deps) {
        (Some(task_name), true) => Some(crate::config::selection::expand_for_mode(
            &config,
            std::slice::from_ref(task_name),
            ctx.mode,
            true,
        )?),
        _ => None,
    };

    // Resolve the sub-config's directory for its own relative paths. URLs
    // and unresolvable paths fall back to the parent's config_dir.
    let sub_config_dir = crate::config::resolve_config_dir(&config_str, &ctx.config_dir);

    let runner = crate::engine::runner::TaskRunner::new(config, ctx.mode, Arc::clone(&ctx.events))
        .with_gate(Arc::clone(&ctx.gate))
        .with_config_dir(sub_config_dir)
        .with_depth(ctx.depth + 1);

    match (run_set, &args.task) {
        (Some(tasks), _) => runner.run_tasks(&tasks, args.force).await,
        (None, Some(task_name)) => runner.run_single_task(task_name, args.force).await,
        (None, None) => runner.run_all(args.force).await,
    }
}
