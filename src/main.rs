use gsim_rs::run;

fn main() {
    env_logger::init();

    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
