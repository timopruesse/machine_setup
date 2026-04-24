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

    // For URLs we pass the string through to load_config. For local paths
    // we keep the resolved PathBuf so canonicalize below can reuse it
    // without re-parsing a string representation.
    let (config_str, config_path): (std::borrow::Cow<'_, str>, Option<std::path::PathBuf>) =
        if is_url {
            (std::borrow::Cow::Borrowed(&args.config), None)
        } else {
            let path = expand_path(&args.config, Some(&ctx.config_dir));
            let s = path.to_string_lossy().into_owned();
            (std::borrow::Cow::Owned(s), Some(path))
        };

    ctx.log(format!("Loading sub-config: {config_str}"));

    let config = crate::config::load_config(&config_str)?;

    // Resolve the sub-config's directory for its own relative paths.
    // URLs fall back to the parent's config_dir.
    let sub_config_dir = match config_path {
        Some(path) => path
            .canonicalize()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| ctx.config_dir.clone()),
        None => ctx.config_dir.clone(),
    };

    let runner =
        crate::engine::runner::TaskRunner::new(config, ctx.mode.clone(), ctx.event_tx.clone())
            .with_config_dir(sub_config_dir)
            .with_depth(ctx.depth + 1);

    if let Some(task_name) = &args.task {
        runner.run_single_task(task_name, false).await
    } else {
        runner.run_all(false).await
    }
}
