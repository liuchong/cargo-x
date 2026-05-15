#![deny(warnings)]

mod config;
mod handle;
mod meta;

use anyhow::Result;
use std::env;
use std::process::exit;

#[cfg(unix)]
mod sys_cfg {
    pub const MAIN_CMD: &str = "cargo-x";
    pub const SHELL_CMD: &str = "sh";
    pub const SHELL_ARG: &str = "-c";
}

#[cfg(windows)]
mod sys_cfg {
    pub const MAIN_CMD: &str = "cargo-x.exe";
    pub const SHELL_CMD: &str = "cmd.exe";
    pub const SHELL_ARG: &str = "/c";
}

#[cfg(not(any(unix, windows)))]
compile_error!("cargo-x currently supports Unix and Windows targets");

pub fn start() -> Result<()> {
    // parse and verify configuration files first
    let x_conf = config::get()?;

    // To allow running both as `cargo-x` and `cargo x`
    let argv = {
        let mut args = env::args();
        let mut argv = Vec::new();
        argv.push(args.next().unwrap());

        match args.next() {
            None => {}
            Some(ref arg)
                if argv[0].ends_with(sys_cfg::MAIN_CMD) && arg == "x" => {}
            Some(arg) => argv.push(arg),
        }

        argv.extend(args);

        if argv.len() == 2 {
            argv
        } else {
            eprintln!("wrong arguments length");
            exit(1);
        }
    };

    exit(handle::run(&argv[1], &x_conf));
}
