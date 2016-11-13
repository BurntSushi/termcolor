#[macro_use]
extern crate clap;
#[macro_use]
extern crate lazy_static;

use std::fs;

use clap::Shell;

#[allow(dead_code)]
#[path = "src/app.rs"]
mod app;

fn main() {
    fs::create_dir_all(env!("OUT_DIR")).unwrap();

    let mut app = app::app_short();
    app.gen_completions("rg", Shell::Bash, env!("OUT_DIR"));
    app.gen_completions("rg", Shell::Fish, env!("OUT_DIR"));
    // Zsh seems to fail with a panic.
    // app.gen_completions("rg", Shell::Zsh, env!("OUT_DIR"));
    app.gen_completions("rg", Shell::PowerShell, env!("OUT_DIR"));
}
