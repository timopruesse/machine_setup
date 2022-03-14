pub mod commands;

use commands::copy::copy_dir;
use home::home_dir;

fn main() {
    let home = home_dir().unwrap();
    let home_path = home.to_str().unwrap();
    let src_path = format!("{}/install-wsl", home_path);
    let dst_path = format!("{}/install-wsl2", home_path);

    copy_dir(&src_path, &dst_path).unwrap();
}
