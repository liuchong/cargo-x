use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Deserialize)]
struct Cargo {
    package: Option<Package>,
}

#[derive(Deserialize)]
struct Package {
    metadata: Option<Metadata>,
}

#[derive(Deserialize)]
struct Metadata {
    x: Option<Xconf>,
}

pub type Xconf = HashMap<String, String>;

fn read_file(path: &Path) -> Result<String> {
    fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))
}

fn parse_xconf(path: &Path) -> Result<Xconf> {
    if !path.exists() {
        return Ok(HashMap::new());
    }

    toml::from_str(&read_file(path)?)
        .with_context(|| format!("failed to parse {}", path.display()))
}

fn dotx(home_dir: Option<&Path>) -> Result<Xconf> {
    let Some(home_dir) = home_dir else {
        return Ok(HashMap::new());
    };

    parse_xconf(&home_dir.join(".x.toml"))
}

fn x(root: &Path) -> Result<Xconf> {
    parse_xconf(&root.join("x.toml"))
}

fn cargo(root: &Path) -> Result<Xconf> {
    let path = root.join("Cargo.toml");
    if !path.exists() {
        return Ok(HashMap::new());
    }

    let cargo_toml: Cargo = toml::from_str(&read_file(&path)?)
        .with_context(|| format!("failed to parse {}", path.display()))?;

    Ok(cargo_toml
        .package
        .and_then(|package| package.metadata)
        .and_then(|metadata| metadata.x)
        .unwrap_or_default())
}

fn get_from(home_dir: Option<&Path>, root: Option<&Path>) -> Result<Xconf> {
    let mut x_conf = dotx(home_dir)?;

    if let Some(root) = root {
        x_conf.extend(x(root)?);
        x_conf.extend(cargo(root)?);
    }

    if x_conf.contains_key("x") {
        // avoid problem caused by run `cargo-x x` directly
        bail!("command key `x` is reserved");
    }

    Ok(x_conf)
}

pub fn get() -> Result<Xconf> {
    let home_dir = dirs::home_dir();
    let root = super::meta::root()?;

    get_from(home_dir.as_deref(), root.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(label: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        env::temp_dir().join(format!(
            "cargo-x-test-{}-{label}-{nanos}",
            std::process::id()
        ))
    }

    #[test]
    fn merges_configs_with_local_precedence() {
        let home = temp_dir("home");
        let root = temp_dir("root");
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(&root).unwrap();
        fs::write(home.join(".x.toml"), "shared = \"home\"\nhome = \"yes\"\n")
            .unwrap();
        fs::write(root.join("x.toml"), "shared = \"local\"\nlocal = \"yes\"\n")
            .unwrap();
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n\n[package.metadata.x]\nshared = \"cargo\"\ncargo = \"yes\"\n",
        )
        .unwrap();

        let conf = get_from(Some(&home), Some(&root)).unwrap();

        assert_eq!(conf.get("shared").map(String::as_str), Some("cargo"));
        assert_eq!(conf.get("home").map(String::as_str), Some("yes"));
        assert_eq!(conf.get("local").map(String::as_str), Some("yes"));
        assert_eq!(conf.get("cargo").map(String::as_str), Some("yes"));

        fs::remove_dir_all(home).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_reserved_x_command() {
        let home = temp_dir("reserved");
        fs::create_dir_all(&home).unwrap();
        fs::write(home.join(".x.toml"), "x = \"cargo test\"\n").unwrap();

        let err = get_from(Some(&home), None).unwrap_err();

        assert!(err.to_string().contains("command key `x` is reserved"));

        fs::remove_dir_all(home).unwrap();
    }
}
