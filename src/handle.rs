use super::config::{Command, CommandKind, Xconf};
use super::fingerprint;
use super::sys_cfg::{SHELL_ARG, SHELL_CMD};
use std::collections::hash_map::DefaultHasher;
use std::env;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::{Command as ProcCommand, ExitStatus};
use std::time::Duration;

const MAX_ALIAS_DEPTH: usize = 8;
const WATCH_POLL_MS: u64 = 500;

#[derive(Debug, Default, Clone, Copy)]
pub struct Opts {
    pub dry_run: bool,
    pub quiet: bool,
    pub watch: bool,
}

/// Follow alias chains until a real command name is found.
fn resolve<'a>(mut name: &'a str, conf: &'a Xconf) -> Result<&'a str, String> {
    for _ in 0..MAX_ALIAS_DEPTH {
        match conf.aliases.get(name) {
            Some(alias) => name = alias.target.as_str(),
            None => return Ok(name),
        }
    }
    Err(format!("alias chain too long (cycle?) at `{name}`"))
}

#[cfg(unix)]
fn shell_escape(arg: &str) -> String {
    if arg.is_empty() {
        return "''".to_string();
    }
    if arg
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "-._/:@=+%,".contains(c))
    {
        arg.to_string()
    } else {
        format!("'{}'", arg.replace('\'', "'\\''"))
    }
}

#[cfg(windows)]
fn shell_escape(arg: &str) -> String {
    if arg.is_empty() {
        return "\"\"".to_string();
    }
    if arg
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "-._/:@=+%,".contains(c))
    {
        arg.to_string()
    } else {
        format!("\"{}\"", arg.replace('"', "\\\""))
    }
}

/// Substitute `{{args}}` if present, otherwise append extra args.
pub fn interpolate(tpl: &str, args: &[String]) -> String {
    let joined = args
        .iter()
        .map(|arg| shell_escape(arg))
        .collect::<Vec<_>>()
        .join(" ");

    if tpl.contains("{{args}}") {
        tpl.replace("{{args}}", &joined)
    } else if joined.is_empty() {
        tpl.to_string()
    } else {
        format!("{tpl} {joined}")
    }
}

fn truncate(text: &str, max: usize) -> String {
    let text = text.replace('\n', " ");
    if text.chars().count() <= max {
        text
    } else {
        let cut: String = text.chars().take(max.saturating_sub(1)).collect();
        format!("{cut}…")
    }
}

pub fn list(conf: &Xconf) -> i32 {
    if conf.is_empty() {
        println!("no commands defined");
        println!();
        println!("define commands in ~/.x.toml, ./x.toml, or [package.metadata.x] in Cargo.toml,");
        println!("or run `x --init` to create an example x.toml");
        return 0;
    }

    let width = conf
        .commands
        .keys()
        .chain(conf.aliases.keys())
        .map(|key| key.len())
        .max()
        .unwrap_or(0);

    println!("available commands:");
    for (name, command) in &conf.commands {
        let detail = match &command.desc {
            Some(desc) => desc.clone(),
            None => match &command.kind {
                CommandKind::Shell(cmd) => truncate(cmd, 60),
                CommandKind::Seq(seq) => format!("seq: {}", seq.join(", ")),
            },
        };
        println!("  {name:<width$}  {detail}  ({})", command.source);
    }
    for (name, alias) in &conf.aliases {
        println!("  {name:<width$}  -> {}  ({})", alias.target, alias.source);
    }
    0
}

pub fn show(name: &str, conf: &Xconf) -> i32 {
    let name = match resolve(name, conf) {
        Ok(name) => name,
        Err(err) => {
            eprintln!("{err}");
            return 1;
        }
    };

    let Some(command) = conf.commands.get(name) else {
        eprintln!("no such command <{name}>");
        return 1;
    };

    println!("name:    {name}");
    println!("source:  {}", command.source);
    if let Some(desc) = &command.desc {
        println!("desc:    {desc}");
    }
    match &command.kind {
        CommandKind::Shell(cmd) => println!("cmd:     {cmd}"),
        CommandKind::Seq(seq) => println!("seq:     {}", seq.join(", ")),
    }
    if let Some(cwd) = &command.cwd {
        println!("cwd:     {cwd}");
    }
    for (key, value) in &command.env {
        println!("env:     {key}={value}");
    }
    if !command.deps.is_empty() {
        println!("deps:    {} (parallel)", command.deps.join(", "));
    }
    if command.cache {
        println!("cache:   true");
    }
    if let Some(inputs) = &command.inputs {
        println!("inputs:  {}", inputs.join(", "));
    }
    if command.confirm {
        println!("confirm: true");
    }
    0
}

pub fn comp_commands(conf: &Xconf) {
    for name in conf.commands.keys().chain(conf.aliases.keys()) {
        println!("{name}");
    }
}

fn exec(sys_cmd: &str, command: &Command) -> io::Result<ExitStatus> {
    let mut proc = ProcCommand::new(SHELL_CMD);
    proc.arg(SHELL_ARG).arg(sys_cmd);

    for (key, value) in &command.env {
        proc.env(key, value);
    }

    if let Some(cwd) = &command.cwd {
        let dir = match &command.base_dir {
            Some(base) => base.join(cwd),
            None => PathBuf::from(cwd),
        };
        proc.current_dir(dir);
    }

    proc.status()
}

fn ask_confirm(name: &str, final_cmd: &str) -> bool {
    eprint!("run `{name}`: {final_cmd} ? [y/N] ");
    let _ = io::stderr().flush();

    let mut line = String::new();
    match io::stdin().read_line(&mut line) {
        Ok(_) => matches!(line.trim().to_lowercase().as_str(), "y" | "yes"),
        Err(_) => false,
    }
}

fn cache_file(command: &Command, name: &str) -> Option<PathBuf> {
    let safe: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "-_".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    command
        .base_dir
        .as_ref()
        .map(|base| base.join(".x-cache").join(safe))
}

/// Hash the command's inputs together with the expanded command line.
fn command_fingerprint(command: &Command, final_cmd: &str) -> u64 {
    let base = command
        .base_dir
        .clone()
        .unwrap_or_else(|| PathBuf::from("."));
    let inputs: Vec<PathBuf> = match &command.inputs {
        Some(list) => list.iter().map(|p| base.join(p)).collect(),
        None => vec![base],
    };
    let files = fingerprint::fingerprint_inputs(&inputs);

    let mut hasher = DefaultHasher::new();
    files.hash(&mut hasher);
    final_cmd.hash(&mut hasher);
    hasher.finish()
}

fn run_shell(
    name: &str,
    tpl: &str,
    args: &[String],
    command: &Command,
    opts: &Opts,
) -> i32 {
    let final_cmd = interpolate(tpl, args);

    if opts.dry_run {
        println!("{final_cmd}");
        return 0;
    }

    // Incremental cache: skip when inputs have not changed since the
    // last successful run.
    let mut cache_write: Option<(PathBuf, u64)> = None;
    if command.cache {
        if let Some(path) = cache_file(command, name) {
            let fp = command_fingerprint(command, &final_cmd);
            let hit = fs::read_to_string(&path)
                .ok()
                .and_then(|s| s.trim().parse::<u64>().ok())
                == Some(fp);
            if hit {
                eprintln!("`{name}` is up to date (cached)");
                return 0;
            }
            cache_write = Some((path, fp));
        }
    }

    if command.confirm && !ask_confirm(name, &final_cmd) {
        eprintln!("aborted");
        return 1;
    }

    if !opts.quiet {
        eprintln!("==> {final_cmd}");
    }

    let code = match exec(&final_cmd, command) {
        Ok(status) => status.code().unwrap_or(1),
        Err(err) => {
            eprintln!("failed to execute <{name}>: {err}");
            1
        }
    };

    if code == 0 {
        if let Some((path, fp)) = cache_write {
            if let Some(dir) = path.parent() {
                let _ = fs::create_dir_all(dir);
            }
            let _ = fs::write(&path, fp.to_string());
        }
    }

    code
}

/// Run dependencies in parallel; return the first failure's exit code.
fn run_deps(
    deps: &[String],
    conf: &Xconf,
    opts: &Opts,
    stack: &[String],
) -> i32 {
    if deps.len() == 1 {
        let mut stack = stack.to_vec();
        return run_inner(&deps[0], &[], conf, opts, &mut stack);
    }

    if !opts.quiet {
        eprintln!("==> deps (parallel): {}", deps.join(", "));
    }

    std::thread::scope(|scope| {
        let handles: Vec<_> = deps
            .iter()
            .map(|dep| {
                let mut stack = stack.to_vec();
                scope.spawn(move || run_inner(dep, &[], conf, opts, &mut stack))
            })
            .collect();

        let mut first_failure = 0;
        for handle in handles {
            let code = handle.join().unwrap_or(1);
            if code != 0 && first_failure == 0 {
                first_failure = code;
            }
        }
        first_failure
    })
}

fn run_inner(
    name: &str,
    args: &[String],
    conf: &Xconf,
    opts: &Opts,
    stack: &mut Vec<String>,
) -> i32 {
    let name = match resolve(name, conf) {
        Ok(name) => name,
        Err(err) => {
            eprintln!("{err}");
            return 1;
        }
    };

    let Some(command) = conf.commands.get(name) else {
        eprintln!("no such command <{name}>");
        if !conf.is_empty() {
            eprintln!("run `x --list` to see available commands");
        }
        return 1;
    };

    if stack.iter().any(|seen| seen == name) {
        eprintln!(
            "dependency cycle detected: {} -> {name}",
            stack.join(" -> ")
        );
        return 1;
    }
    stack.push(name.to_string());

    // Dependencies run first, in parallel, and gate the command itself.
    if !command.deps.is_empty() {
        if !args.is_empty() {
            eprintln!(
                "command <{name}> has dependencies; extra arguments are not supported"
            );
            return 1;
        }
        let code = run_deps(&command.deps, conf, opts, stack);
        if code != 0 {
            return code;
        }
    }

    let code = match &command.kind {
        CommandKind::Shell(tpl) => run_shell(name, tpl, args, command, opts),
        CommandKind::Seq(steps) => {
            if !args.is_empty() {
                eprintln!(
                    "command <{name}> is a sequence; extra arguments are not supported"
                );
                return 1;
            }
            let mut code = 0;
            for step in steps {
                code = run_inner(step, &[], conf, opts, stack);
                if code != 0 {
                    break;
                }
            }
            code
        }
    };

    stack.pop();
    code
}

fn watch_dir(name: &str, conf: &Xconf) -> PathBuf {
    let resolved = resolve(name, conf).unwrap_or(name);
    conf.commands
        .get(resolved)
        .and_then(|command| command.base_dir.clone())
        .or_else(|| env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Run the command, then re-run it whenever files change. Never returns.
fn watch(name: &str, args: &[String], conf: &Xconf, opts: &Opts) -> ! {
    eprintln!("watching for changes (ctrl-c to stop)");
    loop {
        let mut stack = Vec::new();
        let _ = run_inner(name, args, conf, opts, &mut stack);

        let dir = watch_dir(name, conf);
        let fp = fingerprint::fingerprint_dir(&dir);
        loop {
            std::thread::sleep(Duration::from_millis(WATCH_POLL_MS));
            if fingerprint::fingerprint_dir(&dir) != fp {
                eprintln!("change detected — re-running `{name}`");
                break;
            }
        }
    }
}

pub fn run(name: &str, args: &[String], conf: &Xconf, opts: &Opts) -> i32 {
    if opts.watch {
        watch(name, args, conf, opts);
    }
    let mut stack = Vec::new();
    run_inner(name, args, conf, opts, &mut stack)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(args: &[&str]) -> Vec<String> {
        args.iter().map(|arg| arg.to_string()).collect()
    }

    #[test]
    fn interpolate_without_args_keeps_template() {
        assert_eq!(interpolate("cargo test", &[]), "cargo test");
    }

    #[test]
    fn interpolate_appends_args_without_placeholder() {
        assert_eq!(
            interpolate("cargo test", &strings(&["--", "--nocapture"])),
            "cargo test -- --nocapture"
        );
    }

    #[test]
    fn interpolate_replaces_placeholder() {
        assert_eq!(
            interpolate(
                "cargo test {{args}} --verbose",
                &strings(&["foo bar"])
            ),
            "cargo test 'foo bar' --verbose"
        );
    }

    #[test]
    fn truncate_shortens_long_text() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("a much longer text", 10).chars().count(), 10);
    }
}
