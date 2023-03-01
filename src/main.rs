use std::env;
use std::io::ErrorKind;

use processes::start_process;
use processes::start_registry;

fn main() {
    let mut port = 8080;
    let registry_addr = match env::var("REGISTRY_ADDR") {
        Ok(registry_addr) => registry_addr,
        Err(_) => format!("0.0.0.0:{}", port),
    };
    // Start registry
    if start_registry(registry_addr.clone()).is_err() {
        println!(
            "Starting regular process, registry located at {}",
            registry_addr
        );
    }

    // If the registry is already started, start a regular process
    while match start_process(port, registry_addr.clone()) {
        Ok(_) => false,
        Err(e) => match e.kind() {
            ErrorKind::AddrInUse => {
                port = port + 1;
                println!("Trying with {}", port);
                start_process(port, registry_addr.clone());
                true
            }
            _ => {
                panic!("Unknown error {}", e);
            }
        },
    } {}
}
