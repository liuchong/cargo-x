# cargo-x

[![Build Status](https://github.com/liuchong/cargo-x/actions/workflows/rust.yml/badge.svg)](https://github.com/liuchong/cargo-x/actions/workflows/rust.yml)
[![APACHE licensed](https://img.shields.io/badge/license-apache%202.0-blue.svg)](./LICENSE-APACHE)
[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE-MIT)
[![crates.io](https://img.shields.io/crates/v/cargo-x.svg)](https://crates.io/crates/cargo-x)
[![Released API docs](https://docs.rs/cargo-x/badge.svg)](https://docs.rs/cargo-x)

A lightweight cargo subcommand that runs project-defined commands — a minimal
task runner living in one TOML file.

It also works with **zero configuration**: inside any project, `x` detects
what the project is and offers sensible commands automatically (see
[Zero-config detection](#zero-config-detection)).

## Usage

Install cargo-x:

```sh
cargo install cargo-x
```

Upgrade an existing install:

```sh
cargo install -f cargo-x
```

Define commands in `x.toml`:

```toml
lint = "cargo clippy --workspace --all-targets -- -D warnings"
test = "cargo test --workspace --all-targets"

# a sequence of other commands, run in order, stop on first failure
ci = ["lint", "test"]

# the detailed form
[release]
cmd = "cargo build --release"
desc = "Optimized build"
confirm = true

[doc]
cmd = "cargo doc --open {{args}}"
desc = "Build docs, extra args are inserted at {{args}}"
env = { RUSTDOCFLAGS = "-D warnings" }
cwd = "."

[alias]
t = "test"
```

You can define commands in these places. Later files override earlier ones
when the same command key is used:

0. auto-detected commands (see below)
1. `~/.x.toml`
2. `x.toml` next to the current package `Cargo.toml`
3. `x.toml` files found walking up from the current directory (nearest wins)
4. `[package.metadata.x]` in the current package `Cargo.toml`

The keys `x` and `alias` are reserved.

## Zero-config detection

Without any configuration file, `x` inspects the project and provides
commands automatically:

| Project file            | Commands offered                                            |
| ----------------------- | ----------------------------------------------------------- |
| `Cargo.toml`            | `build`, `test`, `check`, `clippy`, `fmt`, `doc`, `run`, `bench`, `release`, `update` |
| `package.json`          | every script in `"scripts"` (run with npm / pnpm / yarn / bun, chosen by lockfile) |
| `Makefile`              | every target                                                |
| `justfile`              | every recipe                                                |
| `Taskfile.yml`          | every task                                                  |

Detected commands are the lowest-precedence layer: anything you define
yourself overrides them. List them with `x --list` (source shown as
`auto: ...`), or turn detection off with `x --no-auto <COMMAND>`.

Run a command:

```sh
cargo x test
# or
cargo-x test
# or
x test
```

Extra arguments are appended to the command (or inserted at `{{args}}`):

```sh
x test -- --nocapture
```

## Command reference

```text
x                          List available commands
x <COMMAND> [ARGS]...      Run a command
x -l, --list               List available commands
x -s, --show <COMMAND>     Show the full definition of a command
x -n, --dry-run <COMMAND>  Print the expanded command without running it
x -q, --quiet <COMMAND>    Do not echo the command before running it
x --no-auto <COMMAND>      Disable zero-config command detection
x --init                   Create an example x.toml
x --completions <SHELL>    Print a completion script (bash, zsh, fish)
x -V, --version            Print the version
x -h, --help               Print help
```

## Configuration reference

Each command can be written in three forms:

```toml
# 1. simple — a single shell command
test = "cargo test"

# 2. sequence — a list of other command names
ci = ["lint", "test"]

# 3. detailed — a table with extra fields
[test]
cmd = "cargo test {{args}}"   # or: seq = ["lint", "test"]
desc = "Run all tests"        # shown by `x --list`
env = { RUST_LOG = "debug" }  # extra environment variables
cwd = "crates/core"           # relative to the config file
confirm = true                # ask before running (for dangerous commands)
```

Aliases map one name to another command:

```toml
[alias]
t = "test"
```

Notes:

- Sequences do not accept extra arguments.
- Commands run through the system shell (`sh -c` on Unix, `cmd /c` on
  Windows), so pipes, redirects and `&&` work as usual.
- The exit status of the underlying command is propagated, so `x ci` works
  in CI pipelines.

## License

Licensed under either of these:

* Apache License Version 2.0 [LICENSE-APACHE](LICENSE-APACHE)
* MIT License [LICENSE-MIT](LICENSE-MIT)

## Contributing

Please sign a cla, thanks!
