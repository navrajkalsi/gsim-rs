use clap::Parser;

use gsim_rs::app::App;
use gsim_rs::{config::Config, run};

fn main() {
    let config = Config::parse();

    let mut app = App::new(config);

    // log error and exit
    if let Err(e) = app.run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
