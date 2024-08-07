use failure::{format_err, Error};
use regex::Regex;
use serde::Deserialize;
use std::env;
use std::process::Command;
use std::str::from_utf8;

#[derive(Deserialize)]
pub struct Resolve {
    pub root: Option<String>,
}

#[derive(Deserialize)]
/// Parse `cargo metadata` to get workspace_root
pub struct Metadata {
    // pub workspace_members: Vec<String>,
    pub resolve: Resolve,
    // pub workspace_root: String,
}

fn re() -> Regex {
    Regex::new(r".*\(path\+file://(.*)\)").unwrap()
}

fn metadata() -> Result<Metadata, Error> {
    let cargo = env::var("CARGO").unwrap_or_else(|_| String::from("cargo"));

    let mut cmd = Command::new(cargo);
    cmd.arg("metadata");
    let output = cmd.output()?;

    let stdout = from_utf8(&output.stdout)?.trim_end();
    if stdout == "" {
        let stderr = from_utf8(&output.stderr)?.trim_end();
        return Err(format_err!("{}", stderr));
    }

    match serde_json::from_str(stdout) {
        Ok(meta) => Ok(meta),
        Err(e) => Err(format_err!("Bad metadata format: {}", e)),
    }
}

pub fn root() -> Result<String, Error> {
    let meta = metadata()?;

    match &meta.resolve.root {
        Some(r) => match re().captures(r) {
            Some(cap) => Ok(cap[1].to_string()),
            _ => Err(format_err!("Bad root format")),
        },
        _ => Err(format_err!("No root")),
    }
}
