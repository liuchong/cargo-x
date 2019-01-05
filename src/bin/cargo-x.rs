extern crate cargo_x;

fn main() {
    cargo_x::start().unwrap_or_else(|err| {
        eprintln!("{}", err);
        std::process::exit(1);
    });
}
