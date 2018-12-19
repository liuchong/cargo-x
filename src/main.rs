// #![deny(warnings)]

extern crate dirs;
extern crate toml;
#[macro_use]
extern crate serde_derive;

mod handle;
mod parse;

use std::env;
use std::process::exit;

fn main() {
    let mut args = env::args();
    let cmd = match args.nth(2) {
        Some(s) => s,
        None => {
            eprintln!("bad argument");
            exit(1);
        }
    };

    handle::run(&cmd);
}
