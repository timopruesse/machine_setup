pub mod commands;

use commands::copy::copy_dir;

fn main() {
    println!("Hello, world!");
    copy_dir("src/no", "dest").unwrap();
}
