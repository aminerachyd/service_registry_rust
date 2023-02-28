use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
};

use crate::events::{ProcessEvent, RegistryEvent};

pub struct Process {
    id: u32,
    address: String,
}

fn log(str: &str) {
    println!("[Process] {}", str);
}

impl Process {
    pub fn new() -> Self {
        Process {
            id: 0,
            address: String::from(""),
        }
    }

    pub fn run(mut self, port: u32) -> std::io::Result<()> {
        let listener = TcpListener::bind(format!("127.0.0.1:{}", port))?;
        log(&format!("Started process on port {}", port));
        self.address = format!("127.0.0.1:{}", port);

        self.connect_to_registry();

        for stream in listener.incoming() {
            let mut buffer = [0u8; 100];

            let mut stream = stream.unwrap();
            let data_size = stream.read(&mut buffer).unwrap();

            if data_size > 0 {
                let registry_event = handle_buffer(buffer, data_size);

                self.handle_registry_event(registry_event);
            }
        }
        Ok(())
    }

    fn connect_to_registry(&self) {
        log("Connecting to registry...");
        let mut stream = TcpStream::connect("127.0.0.1:8080").unwrap();
        let connect_event = &ProcessEvent::CONNECT {
            addr: self.address.clone(),
        }
        .as_bytes_vec()[..];
        stream.write(connect_event).unwrap();
    }

    fn handle_registry_event(&mut self, registry_event: Option<RegistryEvent>) {
        if registry_event.is_none() {
            log("Data is None");
        } else {
            let registry_event = registry_event.unwrap();

            match registry_event {
                RegistryEvent::REGISTERED { id } => {
                    self.id = id;
                    log(&format!("Connected to registry, given id: {}", id));
                }
            }
        }
    }
}

fn handle_buffer(buffer: [u8; 100], data_size: usize) -> Option<RegistryEvent> {
    RegistryEvent::parse_bytes(&buffer[..data_size])
}
