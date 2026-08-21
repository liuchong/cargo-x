#![deny(warnings)]

mod config;
mod detect;
mod fingerprint;
mod handle;
mod meta;

use anyhow::{bail, Context, Result};
use std::env;
use std::fs;
use std::path::Path;
use std::process::exit;

#[cfg(unix)]
mod sys_cfg {
    pub const MAIN_CMD: &str = "cargo-x";
    pub const SHELL_CMD: &str = "sh";
    pub const SHELL_ARG: &str = "-c";
}

#[cfg(windows)]
mod sys_cfg {
    pub const MAIN_CMD: &str = "cargo-x.exe";
    pub const SHELL_CMD: &str = "cmd.exe";
    pub const SHELL_ARG: &str = "/c";
}

#[cfg(not(any(unix, windows)))]
compile_error!("cargo-x currently supports Unix and Windows targets");

const HELP: &str = "\
cargo-x — run project-defined commands

USAGE:
    x                          List available commands
    x <COMMAND> [ARGS]...      Run a command (extra args are appended)
    x <COMMAND> -- [ARGS]...   Same, `--` separates x flags from args

OPTIONS:
    -l, --list             List available commands
    -s, --show <COMMAND>   Show the full definition of a command
    -n, --dry-run          Print the expanded command without running it
    -q, --quiet            Do not echo the command before running it
    -w, --watch            Re-run the command whenever project files change
        --no-auto          Disable zero-config command detection
        --init             Create an example x.toml in the current directory
        --completions <SH> Print a shell completion script (bash|zsh|fish)
    -V, --version          Print the version
    -h, --help             Print this help

CONFIG:
    Commands are read from these places; later ones win on key conflicts:
      0. auto-detected from the project itself (see below)
      1. ~/.x.toml
      2. x.toml next to the current package Cargo.toml
      3. x.toml files found walking up from the current directory
      4. [package.metadata.x] in the current package Cargo.toml

DETECTION:
    Without any configuration, x still works:
      - cargo projects get build/test/check/clippy/fmt/doc/run/bench/...
      - package.json scripts are bridged (npm/pnpm/yarn/bun by lockfile)
      - Makefile, justfile and Taskfile.yml targets become commands

    Simple:       test = \"cargo test --workspace\"
    Sequence:     ci = [\"lint\", \"test\"]
    Detailed:     [test]
                  cmd = \"cargo test {{args}}\"
                  desc = \"Run tests\"
                  env = { RUST_LOG = \"debug\" }
                  cwd = \"crates/core\"
                  confirm = true
                  deps = [\"build\"]        # run in parallel first, gate on success
                  cache = true            # skip when inputs are unchanged
                  inputs = [\"src\"]        # cache/watch fingerprint inputs
    Aliases:      [alias]
                  t = \"test\"
";

const INIT_TEMPLATE: &str = "\
# cargo-x command definitions.
# Docs: https://github.com/liuchong/cargo-x

lint = \"cargo clippy --workspace --all-targets -- -D warnings\"
test = \"cargo test --workspace --all-targets\"
build = \"cargo build --workspace --all-targets\"

# Run several commands in order, stop on first failure:
# ci = [\"lint\", \"test\"]

# Detailed form:
# [release]
# cmd = \"cargo build --release\"
# desc = \"Optimized build\"
# confirm = true

# Short aliases:
# [alias]
# t = \"test\"
";

const COMPLETIONS_BASH: &str = "\
# bash completion for cargo-x — source this file or save it under
# /etc/bash_completion.d/ or ~/.local/share/bash-completion/completions/x
_x() {
    local cur=\"${COMP_WORDS[COMP_CWORD]}\"
    if [ \"$COMP_CWORD\" -eq 1 ]; then
        COMPREPLY=($(compgen -W \"$(x --comp-commands 2>/dev/null)\" -- \"$cur\"))
    fi
}
complete -F _x x cargo-x
";

const COMPLETIONS_ZSH: &str = "\
#compdef x cargo-x
# zsh completion for cargo-x — save as _x in a directory on your $fpath
_x() {
    if (( CURRENT == 2 )); then
        local -a cmds
        cmds=(${(f)\"$(x --comp-commands 2>/dev/null)\"})
        _describe 'command' cmds
    fi
}
compdef _x x cargo-x
";

const COMPLETIONS_FISH: &str = "\
# fish completion for cargo-x — save as ~/.config/fish/completions/x.fish
complete -c x -n '__fish_use_subcommand' -a '(x --comp-commands 2>/dev/null)'
complete -c cargo-x -n '__fish_use_subcommand' -a '(x --comp-commands 2>/dev/null)'
";

fn completions(shell: &str) -> Result<&'static str> {
    match shell {
        "bash" => Ok(COMPLETIONS_BASH),
        "zsh" => Ok(COMPLETIONS_ZSH),
        "fish" => Ok(COMPLETIONS_FISH),
        other => {
            bail!("unsupported shell `{other}` (expected bash, zsh, or fish)")
        }
    }
}

fn init() -> Result<()> {
    let path = Path::new("x.toml");
    if path.exists() {
        bail!("x.toml already exists in the current directory");
    }

    fs::write(path, INIT_TEMPLATE).context("failed to write x.toml")?;
    println!(
        "created x.toml — edit it, then run `x --list` to see your commands"
    );
    Ok(())
}

pub fn start() -> Result<()> {
    let mut argv: Vec<String> = env::args().collect();

    // To allow running both as `cargo-x` and `cargo x`
    if argv.len() >= 2 && argv[0].ends_with(sys_cfg::MAIN_CMD) && argv[1] == "x"
    {
        argv.remove(1);
    }
    let args = &argv[1..];

    let mut opts = handle::Opts::default();
    let mut auto_detect = true;
    let mut cmd_name: Option<String> = None;
    let mut cmd_args: Vec<String> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];

        // Everything after the command name (or after `--`) belongs to it.
        if cmd_name.is_some() {
            cmd_args.push(arg.clone());
            i += 1;
            continue;
        }

        match arg.as_str() {
            "-h" | "--help" => {
                print!("{HELP}");
                return Ok(());
            }
            "-V" | "--version" => {
                println!("cargo-x {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "-l" | "--list" => {
                let conf = config::get(auto_detect)?;
                exit(handle::list(&conf));
            }
            "-s" | "--show" => {
                let name =
                    args.get(i + 1).context("--show needs a command name")?;
                let name = name.clone();
                let conf = config::get(auto_detect)?;
                exit(handle::show(&name, &conf));
            }
            "--init" => return init(),
            "--completions" => {
                let shell = args
                    .get(i + 1)
                    .context("--completions needs a shell: bash|zsh|fish")?;
                print!("{}", completions(shell)?);
                return Ok(());
            }
            "--comp-commands" => {
                // Hidden flag used by shell completion scripts.
                let conf = config::get(auto_detect)?;
                handle::comp_commands(&conf);
                return Ok(());
            }
            "-n" | "--dry-run" => opts.dry_run = true,
            "-q" | "--quiet" => opts.quiet = true,
            "-w" | "--watch" => opts.watch = true,
            "--no-auto" => auto_detect = false,
            "--" => {
                if let Some(name) = args.get(i + 1) {
                    cmd_name = Some(name.clone());
                    i += 1;
                }
            }
            _ if arg.starts_with('-') && arg.len() > 1 => {
                bail!("unknown option `{arg}`")
            }
            _ => cmd_name = Some(arg.clone()),
        }

        i += 1;
    }

    // A leading `--` in the command args is a separator, not an argument.
    if cmd_args.first().map(String::as_str) == Some("--") {
        cmd_args.remove(0);
    }

    let conf = config::get(auto_detect)?;
    match cmd_name {
        None => exit(handle::list(&conf)),
        Some(name) => exit(handle::run(&name, &cmd_args, &conf, &opts)),
    }
}
