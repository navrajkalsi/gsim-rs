use clap::Parser;

use gsim_rs::app::App;
use gsim_rs::config::Config;

fn main() {
    let config = Config::parse();

    let mut app = match App::build(config) {
        Ok(app) => app,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
}
