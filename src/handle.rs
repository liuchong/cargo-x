use super::config::{Command, CommandKind, Xconf};
use super::sys_cfg::{SHELL_ARG, SHELL_CMD};
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::{Command as ProcCommand, ExitStatus};

const MAX_ALIAS_DEPTH: usize = 8;

#[derive(Debug, Default, Clone, Copy)]
pub struct Opts {
    pub dry_run: bool,
    pub quiet: bool,
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

    if command.confirm && !ask_confirm(name, &final_cmd) {
        eprintln!("aborted");
        return 1;
    }

    if !opts.quiet {
        eprintln!("==> {final_cmd}");
    }

    match exec(&final_cmd, command) {
        Ok(status) => status.code().unwrap_or(1),
        Err(err) => {
            eprintln!("failed to execute <{name}>: {err}");
            1
        }
    }
}

pub fn run(name: &str, args: &[String], conf: &Xconf, opts: &Opts) -> i32 {
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

    match &command.kind {
        CommandKind::Shell(tpl) => run_shell(name, tpl, args, command, opts),
        CommandKind::Seq(steps) => {
            if !args.is_empty() {
                eprintln!("command <{name}> is a sequence; extra arguments are not supported");
                return 1;
            }
            for step in steps {
                let code = run(step, &[], conf, opts);
                if code != 0 {
                    return code;
                }
            }
            0
        }
    }
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
