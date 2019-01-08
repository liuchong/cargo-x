use failure::Error;
use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
struct Cargo {
    package: Package,
}

#[derive(Deserialize)]
struct Package {
    metadata: Option<Metadata>,
}

#[derive(Deserialize)]
struct Metadata {
    x: Option<Xconf>,
}

pub type Xconf = HashMap<String, String>;

fn dotx() -> Result<Xconf, Error> {
    let mut path = match dirs::home_dir() {
        Some(p) => p,
        _ => return Err(format_err!("cannot get home dir")),
    };
    path.push(".x.toml");

    if !Path::new(&path).exists() {
        return Ok(HashMap::new());
    }

    let mut conf_string = String::new();
    File::open(&path).and_then(|mut f| f.read_to_string(&mut conf_string))?;

    let xconf: Xconf = toml::from_str(&conf_string)?;

    Ok(xconf)
}

fn x() -> Result<Xconf, Error> {
    let mut path = PathBuf::new();
    path.push(super::meta::root()?);
    path.push("x.toml");

    if !Path::new(&path).exists() {
        return Ok(HashMap::new());
    }

    let mut conf_string = String::new();
    File::open(&path).and_then(|mut f| f.read_to_string(&mut conf_string))?;

    let xconf: Xconf = toml::from_str(&conf_string)?;

    Ok(xconf)
}

fn cargo() -> Result<Xconf, Error> {
    let mut path = PathBuf::new();
    path.push(super::meta::root()?);
    path.push("Cargo.toml");

    if !Path::new(&path).exists() {
        return Ok(HashMap::new());
    }

    let mut conf_string = String::new();
    File::open(&path).and_then(|mut f| f.read_to_string(&mut conf_string))?;

    let cargo_toml: Cargo = toml::from_str(&conf_string)?;

    Ok(
        if let Some(Metadata { x: Some(x) }) = cargo_toml.package.metadata {
            x
        } else {
            HashMap::new()
        },
    )
}

pub fn get() -> Result<Xconf, Error> {
    let x_conf: Xconf =
        dotx()?.into_iter().chain(x()?).chain(cargo()?).collect();

    for pair in x_conf.clone().into_iter() {
        match pair {
            (ref k, _) if k == "x" => {
                // avoid problem caused by run `cargo-x x` directly
                return Err(format_err!("command key `x` is reserved"));
            }
            _ => {}
        }
    }

    Ok(x_conf)
}
