use anyhow::{anyhow, Context, Result};
use std::env;
use std::path::PathBuf;
use std::process::Command;

pub fn root() -> Result<Option<PathBuf>> {
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());

    let mut cmd = Command::new(cargo);
    cmd.args(["locate-project", "--message-format", "plain"]);
    let output = cmd.output().context("failed to run cargo locate-project")?;

    if !output.status.success() {
        return Ok(None);
    }

    let manifest = String::from_utf8(output.stdout)
        .context("cargo locate-project output was not UTF-8")?;
    let manifest = manifest.trim();
    if manifest.is_empty() {
        return Ok(None);
    }

    let manifest = PathBuf::from(manifest);
    match manifest.parent() {
        Some(root) => Ok(Some(root.to_path_buf())),
        None => Err(anyhow!(
            "bad cargo locate-project output: {}",
            manifest.display()
        )),
    }
}
