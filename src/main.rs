#![deny(warnings)]

extern crate toml;

use std::env;
use std::fs::File;
use std::io::prelude::*;
use std::process::{exit, Command};
use toml::de::Error;
use toml::Value as Toml;

fn exec(sys_cmd: &String) -> () {
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(sys_cmd)
        .spawn()
        .expect("failed to execute");

    let _ecode = child.wait().expect("failed to wait");
}

fn handle(toml: Toml, cmd: &String) -> () {
    match toml {
        Toml::Table(table) => {
            for pair in table.into_iter() {
                match pair {
                    (ref k, Toml::String(ref v)) if k == cmd => {
                        exec(v);
                        return;
                    }
                    _ => {}
                }
            }
        }
        _ => {
            eprintln!("bad toml");
        }
    }
    eprintln!("no such command <{}>", cmd);
}

fn main() {
    let mut args = env::args();
    let cmd = match args.nth(2) {
        Some(s) => s,
        None => {
            eprintln!("bad argument");
            exit(1);
        }
    };

    let mut x_conf = String::new();
    File::open("x.toml")
        .and_then(|mut f| f.read_to_string(&mut x_conf))
        .unwrap();

    let x_r: Result<Toml, Error> = x_conf.parse();
    match x_r {
        Ok(toml) => handle(toml, &cmd),
        Err(error) => eprintln!("failed to parse TOML: {}", error),
    }
}
