use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use std::path::Path;

#[derive(Deserialize)]
struct Cargo {
    package: Package,
}

#[derive(Deserialize)]
struct Package {
    metadata: Metadata,
}

#[derive(Deserialize)]
struct Metadata {
    x: Xconf,
}

type Xconf = HashMap<String, String>;

fn dotx() -> Xconf {
    let mut path = dirs::home_dir().expect("cannot get home dir");
    path.push(".x.toml");

    if !Path::new(&path).exists() {
        return HashMap::new();
    }

    let mut conf_string = String::new();
    File::open(path)
        .and_then(|mut f| f.read_to_string(&mut conf_string))
        .expect("no configuration file ~/.x.toml");

    toml::from_str(&conf_string).expect("~/.x.toml parsing failed")
}

fn x() -> Xconf {
    let path = "x.toml";

    if !Path::new(&path).exists() {
        return HashMap::new();
    }

    let mut conf_string = String::new();
    File::open(path)
        .and_then(|mut f| f.read_to_string(&mut conf_string))
        .expect("no configuration file x.toml");

    toml::from_str(&conf_string).expect("x.toml parsing failed")
}

fn cargo() -> Xconf {
    let path = "Cargo.toml";

    if !Path::new(&path).exists() {
        return HashMap::new();
    }

    let mut conf_string = String::new();
    File::open(path)
        .and_then(|mut f| f.read_to_string(&mut conf_string))
        .expect("no configuration file Cargo.toml");

    let cargo_toml: Cargo = toml::from_str(&conf_string).expect("Cargo.toml parsing failed");
    cargo_toml.package.metadata.x
}

pub fn conf() -> Xconf {
    dotx().into_iter().chain(x()).chain(cargo()).collect()
}
