use clap::IntoApp;
use clap_complete::{
    generate_to,
    shells::{Bash, Elvish, Fish, PowerShell, Zsh},
};

include!("src/terminal/cli.rs");

fn main() {
    let mut command = Args::command();
    command.set_bin_name("machine_setup");

    let outdir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("completions/");

    generate_to(Bash, &mut command, "machine_setup", &outdir).ok();
    generate_to(Zsh, &mut command, "machine_setup", &outdir).ok();
    generate_to(Fish, &mut command, "machine_setup", &outdir).ok();
    generate_to(PowerShell, &mut command, "machine_setup", &outdir).ok();
    generate_to(Elvish, &mut command, "machine_setup", &outdir).ok();
}
