use async_trait::async_trait;

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
    async fn install(&self, ctx: &CommandContext) -> Result<()> {
        run_sub_config(&self.args, ctx).await
    }

    async fn update(&self, ctx: &CommandContext) -> Result<()> {
        run_sub_config(&self.args, ctx).await
    }

    async fn uninstall(&self, ctx: &CommandContext) -> Result<()> {
        run_sub_config(&self.args, ctx).await
    }

    fn description(&self) -> String {
        let mut desc = format!("machine_setup: {}", self.args.config);
        if let Some(task) = &self.args.task {
            desc.push_str(&format!(" (task: {task})"));
        }
        desc
    }
}

async fn run_sub_config(args: &MachineSetupArgs, ctx: &CommandContext) -> Result<()> {
    let is_url = args.config.starts_with("http://") || args.config.starts_with("https://");

    let config_str = if is_url {
        args.config.clone()
    } else {
        let config_path = expand_path(&args.config, Some(&ctx.config_dir));
        config_path.to_string_lossy().to_string()
    };

    ctx.log(format!("Loading sub-config: {config_str}"));

    let config = crate::config::load_config(&config_str)?;

    // Resolve the sub-config's directory for its own relative paths
    // URLs fall back to parent's config_dir
    let sub_config_dir = if is_url {
        ctx.config_dir.clone()
    } else {
        std::path::Path::new(&config_str)
            .canonicalize()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| ctx.config_dir.clone())
    };

    let runner = crate::engine::runner::TaskRunner::new(config, ctx.mode, ctx.event_tx.clone())
        .with_config_dir(sub_config_dir)
        .with_depth(ctx.depth + 1);

    if let Some(task_name) = &args.task {
        runner.run_single_task(task_name, false).await
    } else {
        runner.run_all(false).await
    }
}
