use std::collections::HashMap;

pub struct Cli {
    /// Turn debugging information on
    pub debug: u8,
}

pub fn parse_args() -> Result<HashMap<String, String>, ()> {
    let args: Vec<_> = std::env::args().collect();

    print!("{}[2J", 27 as char);

    Ok(HashMap::new())
}

// maybe traits for each debug class.
// then each thing that needs to be debugged needs to be implemented with a debug level.
//
// have all output in the simulator mod
//
// have a permanent section for alarms
