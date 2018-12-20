use std::env;
use std::process::Command;
use std::str::from_utf8;

#[derive(Deserialize)]
/// Parse `cargo metadata` to get workspace_root
pub struct Metadata {
    pub workspace_root: String,
}

fn metadata() -> Metadata {
    let cargo = env::var("CARGO").unwrap_or_else(|_| String::from("cargo"));

    let mut cmd = Command::new(cargo);
    cmd.arg("metadata");

    let output = cmd.output().expect("failed to run cargo metadata");
    let stdout = from_utf8(&output.stdout).expect("failed to read stdout");

    serde_json::from_str(stdout).expect("failed to parse metadata")
}

pub fn workspace_root() -> String {
    metadata().workspace_root
}
