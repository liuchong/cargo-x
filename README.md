cargo-x
=======

[![Build Status](https://api.travis-ci.org/liuchong/cargo-x.svg?branch=master)](https://travis-ci.org/liuchong/cargo-x)

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

3. use cargo-x

```
cargo x ls
# or
cargo-x ls
```

## License

[MIT](LICENSE)
