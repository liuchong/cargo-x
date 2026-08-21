//! Zero-config command detection.
//!
//! When a project has no (or incomplete) x configuration, useful commands
//! are derived from whatever the project already is: a cargo project gets
//! build/test/clippy/..., package.json scripts are bridged, and Makefile /
//! justfile / Taskfile targets become x commands.
//!
//! Detected commands form the lowest-precedence config layer: any command
//! defined in ~/.x.toml, x.toml, or [package.metadata.x] overrides them.

use super::config::{is_reserved, Command, CommandKind, Xconf};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

/// Commands offered automatically inside a cargo project.
const CARGO_COMMANDS: &[(&str, &str)] = &[
    ("build", "cargo build {{args}}"),
    ("test", "cargo test {{args}}"),
    ("check", "cargo check {{args}}"),
    ("clippy", "cargo clippy {{args}}"),
    ("fmt", "cargo fmt {{args}}"),
    ("doc", "cargo doc --open {{args}}"),
    ("run", "cargo run {{args}}"),
    ("bench", "cargo bench {{args}}"),
    ("release", "cargo build --release"),
    ("update", "cargo update"),
];

const MAKEFILES: &[&str] = &["Makefile", "makefile", "GNUmakefile"];
const JUSTFILES: &[&str] = &["justfile", "Justfile", ".justfile"];
const TASKFILES: &[&str] = &["Taskfile.yml", "Taskfile.yaml", "taskfile.yml"];

fn shell_command(cmd: &str, source: &str, base_dir: &Path) -> Command {
    Command {
        kind: CommandKind::Shell(cmd.to_string()),
        desc: None,
        env: BTreeMap::new(),
        cwd: None,
        confirm: false,
        deps: Vec::new(),
        cache: false,
        inputs: None,
        source: source.to_string(),
        base_dir: Some(base_dir.to_path_buf()),
    }
}

fn insert(
    conf: &mut Xconf,
    name: &str,
    cmd: String,
    source: &str,
    base_dir: &Path,
) {
    if is_reserved(name) {
        return;
    }
    conf.commands
        .insert(name.to_string(), shell_command(&cmd, source, base_dir));
}

/// Detect the package manager from lockfiles, defaulting to npm.
fn package_manager(dir: &Path) -> &'static str {
    if dir.join("pnpm-lock.yaml").exists() {
        "pnpm"
    } else if dir.join("yarn.lock").exists() {
        "yarn"
    } else if dir.join("bun.lockb").exists() || dir.join("bun.lock").exists() {
        "bun"
    } else {
        "npm"
    }
}

/// Bridge package.json scripts as x commands.
fn bridge_package_json(dir: &Path, conf: &mut Xconf) {
    let Ok(text) = fs::read_to_string(dir.join("package.json")) else {
        return;
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
        return;
    };
    let Some(scripts) = json.get("scripts").and_then(|s| s.as_object()) else {
        return;
    };

    let pm = package_manager(dir);
    for name in scripts.keys() {
        // npm requires `--` before forwarded args; pnpm/yarn/bun pass them
        // through directly.
        let cmd = if pm == "npm" {
            format!("{pm} run {name} -- {{{{args}}}}")
        } else {
            format!("{pm} run {name} {{{{args}}}}")
        };
        insert(conf, name, cmd, "auto: package.json", dir);
    }
}

/// Extract target names from Makefile text.
pub fn parse_make_targets(text: &str) -> Vec<String> {
    let mut targets = Vec::new();
    for line in text.lines() {
        if line.starts_with(char::is_whitespace) || line.starts_with('#') {
            continue;
        }
        let Some(colon) = line.find(':') else {
            continue;
        };
        // VAR := value / VAR ::= value are assignments, not targets
        if line[colon + 1..].starts_with('=') {
            continue;
        }
        let name = line[..colon].trim();
        if name.is_empty()
            || name.starts_with('.')
            || name.contains(['%', ' '])
            || !name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
        {
            continue;
        }
        targets.push(name.to_string());
    }
    targets.sort();
    targets.dedup();
    targets
}

/// Extract recipe names from justfile text.
pub fn parse_just_recipes(text: &str) -> Vec<String> {
    let mut recipes = Vec::new();
    for line in text.lines() {
        let line = line.strip_prefix('@').unwrap_or(line);
        if line.starts_with(char::is_whitespace) || line.starts_with('#') {
            continue;
        }
        if line.contains(":=") {
            continue; // assignment
        }
        let Some(colon) = line.find(':') else {
            continue;
        };
        let head = &line[..colon];
        let name = head.split_whitespace().next().unwrap_or("");
        if name.is_empty()
            || matches!(name, "set" | "alias" | "import" | "mod" | "export")
            || !name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "-_".contains(c))
        {
            continue;
        }
        recipes.push(name.to_string());
    }
    recipes.sort();
    recipes.dedup();
    recipes
}

/// Extract task names from Taskfile.yml text (one indent level under `tasks:`).
pub fn parse_taskfile_tasks(text: &str) -> Vec<String> {
    let mut tasks = Vec::new();
    let mut in_tasks = false;
    for line in text.lines() {
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        if !line.starts_with(char::is_whitespace) {
            in_tasks = line.trim_end() == "tasks:";
            continue;
        }
        if !in_tasks {
            continue;
        }
        // task names sit at exactly one indent level under `tasks:`
        let level_one = (line.starts_with("  ") && !line.starts_with("   "))
            || (line.starts_with('\t') && !line.starts_with("\t\t"));
        if !level_one {
            continue;
        }
        let name = line.trim().trim_end_matches(':');
        if !name.is_empty()
            && !name.contains(' ')
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "-_".contains(c))
        {
            tasks.push(name.to_string());
        }
    }
    tasks.sort();
    tasks.dedup();
    tasks
}

fn bridge_first_existing(
    dir: &Path,
    names: &[&str],
    parse: fn(&str) -> Vec<String>,
    tool: &str,
    conf: &mut Xconf,
) {
    for file in names {
        let path = dir.join(file);
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let source = format!("auto: {tool}");
        for name in parse(&text) {
            let cmd = format!("{tool} {name} {{{{args}}}}");
            insert(&mut *conf, &name, cmd, &source, dir);
        }
        return;
    }
}

/// Build the detected-command layer for a project directory.
///
/// `root` is the cargo project root when inside one; everything is probed
/// in that directory, otherwise in `cwd`.
pub fn detect(root: Option<&Path>, cwd: &Path) -> Xconf {
    let mut conf = Xconf::default();
    let dir = root.unwrap_or(cwd);

    if let Some(root) = root {
        for (name, cmd) in CARGO_COMMANDS {
            insert(&mut conf, name, cmd.to_string(), "auto: cargo", root);
        }
    }

    bridge_package_json(dir, &mut conf);
    bridge_first_existing(
        dir,
        MAKEFILES,
        parse_make_targets,
        "make",
        &mut conf,
    );
    bridge_first_existing(
        dir,
        JUSTFILES,
        parse_just_recipes,
        "just",
        &mut conf,
    );
    bridge_first_existing(
        dir,
        TASKFILES,
        parse_taskfile_tasks,
        "task",
        &mut conf,
    );

    conf
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
        std::env::temp_dir().join(format!(
            "cargo-x-detect-{}-{label}-{nanos}",
            std::process::id()
        ))
    }

    #[test]
    fn parses_make_targets() {
        let text = "\
# comment
.PHONY: test
VAR := value
BUILD_DIR = out

build: src/main.rs
\tcc -o out src/main.rs

test: ## run tests
\tcargo test

%.o: %.c
\tcc -c $<
";
        let targets = parse_make_targets(text);
        assert!(targets.contains(&"build".to_string()));
        assert!(targets.contains(&"test".to_string()));
        assert!(!targets.contains(&"VAR".to_string()));
        assert!(!targets.contains(&"BUILD_DIR".to_string()));
        assert!(!targets.iter().any(|t| t.starts_with('.')));
        assert!(!targets.iter().any(|t| t.contains('%')));
    }

    #[test]
    fn parses_just_recipes() {
        let text = "\
set shell := [\"sh\", \"-c\"]

# run tests
test pattern=\"\":
    cargo test {{pattern}}

@lint:
    cargo clippy

build-all: build test
    echo done
";
        let recipes = parse_just_recipes(text);
        assert_eq!(recipes, vec!["build-all", "lint", "test"]);
    }

    #[test]
    fn parses_taskfile_tasks() {
        let text = "\
version: '3'

tasks:
  build:
    cmds:
      - go build ./...
  test:
    deps: [build]
    cmds:
      - go test ./...

vars:
  FOO: bar
";
        let tasks = parse_taskfile_tasks(text);
        assert_eq!(tasks, vec!["build", "test"]);
    }

    #[test]
    fn bridges_package_json_scripts_with_npm() {
        let dir = temp_dir("npm");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("package.json"),
            r#"{"scripts": {"build": "tsc", "dev": "vite"}}"#,
        )
        .unwrap();

        let conf = detect(None, &dir);
        let build = conf.commands.get("build").unwrap();
        assert_eq!(
            build.kind,
            CommandKind::Shell("npm run build -- {{args}}".to_string())
        );
        assert_eq!(build.source, "auto: package.json");
        assert!(conf.commands.contains_key("dev"));

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn pnpm_lockfile_switches_package_manager() {
        let dir = temp_dir("pnpm");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("package.json"), r#"{"scripts": {"dev": "vite"}}"#)
            .unwrap();
        fs::write(dir.join("pnpm-lock.yaml"), "").unwrap();

        let conf = detect(None, &dir);
        assert_eq!(
            conf.commands.get("dev").map(|c| &c.kind),
            Some(&CommandKind::Shell("pnpm run dev {{args}}".to_string()))
        );

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn detects_cargo_commands_at_root() {
        let dir = temp_dir("cargo");
        fs::create_dir_all(&dir).unwrap();

        let conf = detect(Some(&dir), &dir);
        assert!(conf.commands.contains_key("build"));
        assert!(conf.commands.contains_key("test"));
        assert!(conf.commands.contains_key("clippy"));
        assert_eq!(
            conf.commands.get("test").map(|c| c.source.as_str()),
            Some("auto: cargo")
        );

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn skips_reserved_script_names() {
        let dir = temp_dir("reserved");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("package.json"), r#"{"scripts": {"x": "echo no"}}"#)
            .unwrap();

        let conf = detect(None, &dir);
        assert!(!conf.commands.contains_key("x"));

        fs::remove_dir_all(dir).unwrap();
    }
}
