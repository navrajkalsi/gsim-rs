use clap::Parser;

use gsim_rs::{config::Config, run};

fn main() {
    let config = Config::parse();

    // log error and exit
    if let Err(e) = run(config) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
