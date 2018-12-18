cargo-x
=======

A very simple third-party cargo subcommand to execute a custom command

## Usage

1. install cargo-x

```
cargo install -f cargo-x
```

2. write a config file x.toml in "name = detail" format like

```
ls = "ls -ltr"
```

3. use cargo-x

```
cargo x ls
```

## License

[MIT](LICENSE)
