use regex::Regex;
use std::env;
use std::process::Command;
use std::str::from_utf8;

#[derive(Debug, Deserialize)]
pub struct Resolve {
    pub root: Option<String>,
}

#[derive(Debug, Deserialize)]
/// Parse `cargo metadata` to get workspace_root
pub struct Metadata {
    pub workspace_members: Vec<String>,
    pub resolve: Resolve,
    pub workspace_root: String,
}

fn re() -> Regex {
    Regex::new(r".*\(path\+file://(.*)\)").unwrap()
}

fn metadata() -> Metadata {
    let cargo = env::var("CARGO").unwrap_or_else(|_| String::from("cargo"));

    let mut cmd = Command::new(cargo);
    cmd.arg("metadata");

    let output = cmd.output().expect("failed to run cargo metadata");
    let stdout = from_utf8(&output.stdout).expect("failed to read stdout");

    serde_json::from_str(stdout).expect("failed to parse metadata")
}

pub fn root() -> Option<String> {
    match &metadata().resolve.root {
        Some(r) => match re().captures(r) {
            Some(cap) => Some(cap[1].to_string()),
            _ => None,
        },
        _ => None,
    }
}
