cargo-x
=======

[![Build Status](https://api.travis-ci.org/liuchong/cargo-x.svg?branch=master)](https://travis-ci.org/liuchong/cargo-x)
[![APACHE licensed](https://img.shields.io/badge/license-apache%202.0-blue.svg)](./LICENSE-APACHE)
[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE-MIT)
[![crates.io](https://meritbadge.herokuapp.com/cargo-x)](https://crates.io/crates/cargo-x)
[![Released API docs](https://docs.rs/cargo-x/badge.svg)](https://docs.rs/cargo-x)

A very simple third-party cargo subcommand to execute a custom command

## Usage

1. install cargo-x

```
cargo install cargo-x
```

or upgrade

```
cargo install -f cargo-x
```

2. write a config file `x.toml` in `"name = detail"` format like

```
ls = "ls -ltr"
```

or same lines in file `~/.x.toml`,

or in `[package.metadata.x]` section in `Cargo.toml` file.

***note*** that `DO NOT` use key x like `x = "any command"`, `cargo-x x` will run into problem,
because it does not know if it is using `cargo-x x` or `cargo x`.

3. use cargo-x

```
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

### Contributing

Please sign a cla, thanks!
