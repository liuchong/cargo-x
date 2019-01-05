#![deny(warnings)]

extern crate dirs;
#[macro_use]
extern crate failure;
extern crate regex;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate toml;

mod config;
mod handle;
mod meta;

use failure::Error;
use std::env;
use std::process::exit;

#[cfg(any(unix))]
mod sys_cfg {
    pub const MAIN_CMD: &str = "cargo-x";
    pub const SHELL_CMD: &str = "sh";
    pub const SHELL_ARG: &str = "-c";
}

#[cfg(any(windows))]
mod sys_cfg {
    pub const MAIN_CMD: &str = "cargo-x.exe";
    pub const SHELL_CMD: &str = "cmd.exe";
    pub const SHELL_ARG: &str = "/c";
}

pub fn start() -> Result<(), Error> {
    // parse and verify configuration files first
    let x_conf = config::get();

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

    match handle::run(&argv[1], x_conf?) {
        Some(code) => exit(code),
        None => exit(1),
    }
}
