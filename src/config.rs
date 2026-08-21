use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// Keys that cannot be used as command or alias names.
const RESERVED_KEYS: [&str; 2] = ["x", "alias"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandKind {
    /// A single shell command template.
    Shell(String),
    /// A sequence of other command names, run in order, stop on failure.
    Seq(Vec<String>),
}

#[derive(Debug, Clone)]
pub struct Command {
    pub kind: CommandKind,
    pub desc: Option<String>,
    pub env: BTreeMap<String, String>,
    /// Working directory, relative to the config file that defined it.
    pub cwd: Option<String>,
    pub confirm: bool,
    /// Human-readable label of where this command was defined.
    pub source: String,
    /// Directory of the config file that defined this command.
    pub base_dir: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct Alias {
    pub target: String,
    pub source: String,
}

#[derive(Debug, Default)]
pub struct Xconf {
    pub commands: BTreeMap<String, Command>,
    pub aliases: BTreeMap<String, Alias>,
}

impl Xconf {
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty() && self.aliases.is_empty()
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawDef {
    /// `test = "cargo test"`
    Simple(String),
    /// `ci = ["lint", "test"]`
    Seq(Vec<String>),
    /// `[test]` table with fields.
    Detailed(RawDetailed),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDetailed {
    cmd: Option<String>,
    seq: Option<Vec<String>>,
    desc: Option<String>,
    env: Option<BTreeMap<String, String>>,
    cwd: Option<String>,
    confirm: Option<bool>,
}

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
    x: Option<toml::value::Table>,
}

fn read_file(path: &Path) -> Result<String> {
    fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))
}

fn check_reserved(key: &str, source: &str) -> Result<()> {
    if RESERVED_KEYS.contains(&key) {
        bail!("command key `{key}` is reserved ({source})");
    }
    Ok(())
}

fn parse_table(
    table: toml::value::Table,
    source: &str,
    base_dir: Option<PathBuf>,
    conf: &mut Xconf,
) -> Result<()> {
    for (key, value) in table {
        if key == "alias" {
            let aliases: BTreeMap<String, String> =
                value.try_into().with_context(|| {
                    format!("invalid [alias] table in {source}")
                })?;
            for (name, target) in aliases {
                check_reserved(&name, source)?;
                conf.aliases.insert(
                    name,
                    Alias {
                        target,
                        source: source.to_string(),
                    },
                );
            }
            continue;
        }

        check_reserved(&key, source)?;

        let raw: RawDef = value
            .try_into()
            .with_context(|| format!("invalid command `{key}` in {source}"))?;

        let command = match raw {
            RawDef::Simple(cmd) => Command {
                kind: CommandKind::Shell(cmd),
                desc: None,
                env: BTreeMap::new(),
                cwd: None,
                confirm: false,
                source: source.to_string(),
                base_dir: base_dir.clone(),
            },
            RawDef::Seq(seq) => {
                if seq.is_empty() {
                    bail!("command `{key}` in {source} is an empty sequence");
                }
                Command {
                    kind: CommandKind::Seq(seq),
                    desc: None,
                    env: BTreeMap::new(),
                    cwd: None,
                    confirm: false,
                    source: source.to_string(),
                    base_dir: base_dir.clone(),
                }
            }
            RawDef::Detailed(d) => {
                let kind = match (d.cmd, d.seq) {
                    (Some(cmd), None) => CommandKind::Shell(cmd),
                    (None, Some(seq)) => {
                        if seq.is_empty() {
                            bail!("command `{key}` in {source} is an empty sequence");
                        }
                        CommandKind::Seq(seq)
                    }
                    _ => bail!(
                        "command `{key}` in {source} needs exactly one of `cmd` or `seq`"
                    ),
                };
                Command {
                    kind,
                    desc: d.desc,
                    env: d.env.unwrap_or_default(),
                    cwd: d.cwd,
                    confirm: d.confirm.unwrap_or(false),
                    source: source.to_string(),
                    base_dir: base_dir.clone(),
                }
            }
        };

        conf.commands.insert(key, command);
    }
    Ok(())
}

fn load_x_file(path: &Path, source: &str, conf: &mut Xconf) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let table: toml::value::Table = toml::from_str(&read_file(path)?)
        .with_context(|| format!("failed to parse {}", path.display()))?;

    parse_table(table, source, path.parent().map(Path::to_path_buf), conf)
}

fn load_cargo_metadata(root: &Path, conf: &mut Xconf) -> Result<()> {
    let path = root.join("Cargo.toml");
    if !path.exists() {
        return Ok(());
    }

    let cargo_toml: Cargo = toml::from_str(&read_file(&path)?)
        .with_context(|| format!("failed to parse {}", path.display()))?;

    let table = cargo_toml
        .package
        .and_then(|package| package.metadata)
        .and_then(|metadata| metadata.x);

    if let Some(table) = table {
        parse_table(
            table,
            "Cargo.toml [package.metadata.x]",
            Some(root.to_path_buf()),
            conf,
        )?;
    }

    Ok(())
}

/// Merge configs from the home directory and the given project root.
/// Kept separate from `get` so unit tests do not depend on the cwd.
fn get_from(home_dir: Option<&Path>, root: Option<&Path>) -> Result<Xconf> {
    let mut conf = Xconf::default();

    if let Some(home_dir) = home_dir {
        load_x_file(&home_dir.join(".x.toml"), "~/.x.toml", &mut conf)?;
    }

    if let Some(root) = root {
        let x_path = root.join("x.toml");
        load_x_file(&x_path, &x_path.display().to_string(), &mut conf)?;
    }

    Ok(conf)
}

/// Load all configuration sources. Later sources override earlier ones
/// when the same command key is used:
///
/// 1. `~/.x.toml`
/// 2. `x.toml` next to the current package `Cargo.toml`
/// 3. `x.toml` files found walking up from the cwd (nearest wins)
/// 4. `[package.metadata.x]` in the current package `Cargo.toml`
pub fn get() -> Result<Xconf> {
    let home_dir = dirs::home_dir();
    let root = super::meta::root()?;

    let mut conf = get_from(home_dir.as_deref(), root.as_deref())?;

    // Walk up from the cwd, collecting x.toml files below the project root
    // (or all the way up when outside a cargo project). Nearest file wins.
    if let Ok(cwd) = env::current_dir() {
        let mut chain: Vec<PathBuf> = Vec::new();
        let mut dir = Some(cwd.as_path());
        while let Some(d) = dir {
            if root.as_deref() == Some(d) {
                break; // root x.toml already loaded by get_from
            }
            let candidate = d.join("x.toml");
            if candidate.exists() {
                chain.push(candidate);
            }
            dir = d.parent();
        }
        for path in chain.iter().rev() {
            load_x_file(path, &path.display().to_string(), &mut conf)?;
        }
    }

    if let Some(root) = root {
        load_cargo_metadata(&root, &mut conf)?;
    }

    Ok(conf)
}

#[cfg(test)]
mod tests {
    use super::*;
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

    fn shell(cmd: &Command) -> Option<&str> {
        match &cmd.kind {
            CommandKind::Shell(cmd) => Some(cmd),
            CommandKind::Seq(_) => None,
        }
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

        // project metadata is loaded after x.toml in `get`, simulate here
        let mut conf = get_from(Some(&home), Some(&root)).unwrap();
        load_cargo_metadata(&root, &mut conf).unwrap();

        assert_eq!(conf.commands.get("shared").and_then(shell), Some("cargo"));
        assert_eq!(conf.commands.get("home").and_then(shell), Some("yes"));
        assert_eq!(conf.commands.get("local").and_then(shell), Some("yes"));
        assert_eq!(conf.commands.get("cargo").and_then(shell), Some("yes"));

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

    #[test]
    fn parses_detailed_command() {
        let home = temp_dir("detailed");
        fs::create_dir_all(&home).unwrap();
        fs::write(
            home.join(".x.toml"),
            "[test]\ncmd = \"cargo test\"\ndesc = \"Run tests\"\nconfirm = true\ncwd = \"sub\"\n\n[test.env]\nRUST_LOG = \"debug\"\n",
        )
        .unwrap();

        let conf = get_from(Some(&home), None).unwrap();
        let cmd = conf.commands.get("test").unwrap();

        assert_eq!(shell(cmd), Some("cargo test"));
        assert_eq!(cmd.desc.as_deref(), Some("Run tests"));
        assert!(cmd.confirm);
        assert_eq!(cmd.cwd.as_deref(), Some("sub"));
        assert_eq!(cmd.env.get("RUST_LOG").map(String::as_str), Some("debug"));

        fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn parses_sequence_and_alias() {
        let home = temp_dir("seq");
        fs::create_dir_all(&home).unwrap();
        fs::write(
            home.join(".x.toml"),
            "lint = \"cargo clippy\"\ntest = \"cargo test\"\nci = [\"lint\", \"test\"]\n\n[alias]\nt = \"test\"\n",
        )
        .unwrap();

        let conf = get_from(Some(&home), None).unwrap();

        assert_eq!(
            conf.commands.get("ci").map(|c| &c.kind),
            Some(&CommandKind::Seq(vec![
                "lint".to_string(),
                "test".to_string()
            ]))
        );
        assert_eq!(
            conf.aliases.get("t").map(|a| a.target.as_str()),
            Some("test")
        );

        fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn rejects_detailed_without_cmd_or_seq() {
        let home = temp_dir("invalid");
        fs::create_dir_all(&home).unwrap();
        fs::write(home.join(".x.toml"), "[bad]\ndesc = \"nothing to run\"\n")
            .unwrap();

        let err = get_from(Some(&home), None).unwrap_err();

        assert!(err.to_string().contains("exactly one of `cmd` or `seq`"));

        fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn rejects_unknown_detailed_field() {
        let home = temp_dir("unknown-field");
        fs::create_dir_all(&home).unwrap();
        fs::write(home.join(".x.toml"), "[bad]\ncmdd = \"typo\"\n").unwrap();

        let err = get_from(Some(&home), None).unwrap_err();

        assert!(err.to_string().contains("invalid command `bad`"));

        fs::remove_dir_all(home).unwrap();
    }
}
