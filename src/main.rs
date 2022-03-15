pub mod commands;
pub mod config;

// use commands::copy::copy_dir;
// use home::home_dir;

use std::env;

use config::base_config::BaseConfig;
use config::yaml_config::YamlConfig;

fn main() {
    let config = YamlConfig {};
    let result = config.read(&format!(
        "{}/{}",
        env::current_dir().unwrap().to_str().unwrap(),
        "test.yml"
    ));

    // let home = home_dir().unwrap();
    // let home_path = home.to_str().unwrap();
    // let src_path = format!("{}/install-wsl", home_path);
    // let dst_path = format!("{}/install-wsl2", home_path);

    // copy_dir(&src_path, &dst_path).unwrap();
}
