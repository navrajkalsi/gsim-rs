use gsim_rs::run;

fn main() {
    // log error and exit
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
