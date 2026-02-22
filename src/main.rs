use std::fs;

use gsim_rs::{Machine, Point, parse_block};

fn main() {
    let contents = fs::read_to_string("gcode.nc").unwrap();
    println!("{contents}");
    for line in contents.split(';').map(|x| x.trim()) {
        println!("{:?}", parse_block(line).unwrap());
    }

    let travels = Point::new(10.0, 10.0, 30.0);
    let machine = Machine::build(travels).expect("Machine build error");

    println!("Machine: {machine}");
}
