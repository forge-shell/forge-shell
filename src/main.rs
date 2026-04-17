#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

fn main() {
    println!("Forge Shell v{}", env!("CARGO_PKG_VERSION"));
}
