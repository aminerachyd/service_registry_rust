use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
};

use crate::events::{ProcessEvent, RegistryEvent};

pub struct Registry {
    processes: HashMap<u32, String>,
    last_registered_id: u32,
}

fn log(str: &str) {
    println!("[Registry] {}", str);
}

fn handle_buffer(buffer: [u8; 100], data_size: usize) -> Option<ProcessEvent> {
    ProcessEvent::parse_bytes(&buffer[..data_size])
}

impl Registry {
    pub fn new() -> Self {
        let mut processes = HashMap::new();
        // FIXME Hardcoded port
        processes.insert(0, "127.0.0.1:8080".to_owned());
        Registry {
            last_registered_id: 0,
            processes,
        }
    }

    pub fn run(mut self, port: u32) -> std::io::Result<()> {
        let listener = TcpListener::bind(format!("127.0.0.1:{}", port))?;
        log(&format!("Started registry on port {}", port));

        for stream in listener.incoming() {
            let mut buffer = [0u8; 100];

            let mut stream = stream.unwrap();
            let data_size = stream.read(&mut buffer).unwrap();

            if data_size > 0 {
                let process_event = handle_buffer(buffer, data_size);

                self.handle_process_event(process_event);
            }
        }
        Ok(())
    }

    fn handle_process_event(&mut self, process_event: Option<ProcessEvent>) {
        if process_event.is_none() {
            log("Data is None");
        } else {
            let process_event = process_event.unwrap();

            match process_event {
                ProcessEvent::CONNECT { addr } => {
                    log(&format!("Received CONNECT from {}", addr));
                    self.register_process(addr);
                }
            }
        }
    }

    fn register_process(&mut self, addr: String) {
        let mut stream = TcpStream::connect(&addr).unwrap();
        let next_id = self.last_registered_id + 1;
        self.processes.insert(next_id, addr);
        self.last_registered_id = next_id;

        let registry_event = &RegistryEvent::REGISTERED { id: next_id }.as_bytes_vec()[..];
        stream.write(registry_event).unwrap();
    }
}
