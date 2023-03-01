use std::env;
use std::io::ErrorKind;

use processes::start_process;
use processes::start_registry;

fn main() {
    let mut port = 8080;
    let mut registry_addr = format!("0.0.0.0:{port}");
    let mut is_registry = true;

    match env::var("REGISTRY_ADDR") {
        Ok(addr) => {
            registry_addr = addr;
            is_registry = false;
        }
        Err(_) => {}
    };

    // Start registry
    if is_registry == true {
        match start_registry(registry_addr.clone()) {
            Ok(_) => {}
            Err(_) => {
                println!(
                    "Starting regular process, registry located at {}",
                    registry_addr
                );

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
        }
    } else {
        println!(
            "Starting regular process, registry located at {}",
            registry_addr
        );
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
}
