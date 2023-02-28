use std::io::ErrorKind;

use processes::start_process;
use processes::start_registry;

fn main() {
    let mut port = 8080;
    // Start registry
    start_registry(port).unwrap();

    // If the registyr is already started, start a regular process
    while match start_process(port) {
        Ok(_) => false,
        Err(e) => match e.kind() {
            ErrorKind::AddrInUse => {
                port = port + 1;
                println!("Trying with {}", port);
                start_process(port);
                true
            }
            _ => {
                panic!("Unkown error {}", e);
            }
        },
    } {}
}
