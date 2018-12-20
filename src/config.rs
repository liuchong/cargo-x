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
    File::open(&path)
        .and_then(|mut f| f.read_to_string(&mut conf_string))
        .expect(&format!("failed to read {}", path.to_string_lossy()));

    toml::from_str(&conf_string)
        .expect(&format!("{} parsing failed", path.to_string_lossy()))
}

fn x() -> Xconf {
    let mut path = PathBuf::new();
    path.push(super::meta::workspace_root());
    path.push("x.toml");

    if !Path::new(&path).exists() {
        return HashMap::new();
    }

    let mut conf_string = String::new();
    File::open(&path)
        .and_then(|mut f| f.read_to_string(&mut conf_string))
        .expect(&format!("failed to read {}", path.to_string_lossy()));

    toml::from_str(&conf_string)
        .expect(&format!("{} parsing failed", path.to_string_lossy()))
}

fn cargo() -> Xconf {
    let mut path = PathBuf::new();
    path.push(super::meta::workspace_root());
    path.push("Cargo.toml");

    if !Path::new(&path).exists() {
        return HashMap::new();
    }

    let mut conf_string = String::new();
    File::open(&path)
        .and_then(|mut f| f.read_to_string(&mut conf_string))
        .expect(&format!("failed to read {}", path.to_string_lossy()));

    let cargo_toml: Cargo = toml::from_str(&conf_string)
        .expect(&format!("{} parsing failed", path.to_string_lossy()));
    cargo_toml.package.metadata.x
}

pub fn get() -> Xconf {
    dotx().into_iter().chain(x()).chain(cargo()).collect()
}
