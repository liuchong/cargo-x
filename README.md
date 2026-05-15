# cargo-x

[![Build Status](https://github.com/liuchong/cargo-x/actions/workflows/rust.yml/badge.svg)](https://github.com/liuchong/cargo-x/actions/workflows/rust.yml)
[![APACHE licensed](https://img.shields.io/badge/license-apache%202.0-blue.svg)](./LICENSE-APACHE)
[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE-MIT)
[![crates.io](https://img.shields.io/crates/v/cargo-x.svg)](https://crates.io/crates/cargo-x)
[![Released API docs](https://docs.rs/cargo-x/badge.svg)](https://docs.rs/cargo-x)

A very simple third-party cargo subcommand to execute a custom command

## Usage

Install cargo-x:

```sh
cargo install cargo-x
```

Upgrade an existing install:

```sh
cargo install -f cargo-x
```

Add commands in `x.toml`:

```toml
ls = "ls -ltr"
test = "cargo test --workspace --all-targets"
```

You can define commands in these places. Later files override earlier ones
when the same command key is used:

1. `~/.x.toml`
2. `x.toml` next to the current package `Cargo.toml`
3. `[package.metadata.x]` in the current package `Cargo.toml`

The key `x` is reserved. Do not configure `x = "any command"`.

Run a configured command:

```sh
cargo x ls
# or
cargo-x ls
# or
x ls
```

## License

Licensed under either of these:

* Apache License Version 2.0 [LICENSE-APACHE](LICENSE-APACHE)
* MIT License [LICENSE-MIT](LICENSE-MIT)

## Contributing

Please sign a cla, thanks!
