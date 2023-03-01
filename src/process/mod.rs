use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::exit,
    thread,
    time::Duration,
};

use crate::{
    events::{ProcessEvent, RegistryEvent},
    P2PSend,
};

pub struct Process {
    id: u32,
    address: String,
    registry_address: String,
    registered_processes: HashMap<u32, String>,
}

impl P2PSend for Process {}

impl Process {
    pub fn new() -> Self {
        Process {
            id: 0,
            address: String::from(""),
            registry_address: String::from(""),
            registered_processes: HashMap::new(),
        }
    }

    pub fn run(mut self, port: u32, registry_address: String) -> std::io::Result<()> {
        let listener = TcpListener::bind(format!("127.0.0.1:{}", port))?;
        log(&format!("Started process on port {}", port));

        self.address = format!("0.0.0.0:{}", port);
        self.registry_address = registry_address;

        self.connect_to_registry(self.registry_address.clone());

        let registry_address_clone = self.registry_address.clone();

        thread::spawn(move || loop {
            thread::sleep(Duration::from_secs(5));

            Process::send_heartbeat(registry_address_clone.to_owned());
        });

        for stream in listener.incoming() {
            let mut buffer = [0u8; 1000];

            let mut stream = stream.unwrap();
            let data_size = stream.read(&mut buffer).unwrap();

            if data_size > 0 {
                let registry_event = handle_buffer(buffer, data_size);

                self.handle_registry_event(registry_event);
            }
        }
        Ok(())
    }

    fn connect_to_registry(&self, registry_address: String) {
        log("Connecting to registry...");
        let mut stream = TcpStream::connect(registry_address).unwrap();
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
                RegistryEvent::REGISTERED {
                    id,
                    registered_processes,
                } => {
                    self.id = id;
                    self.registered_processes = registered_processes;
                    log(&format!(
                        "Connected to registry, given id: {}\n Registered processes {:?}",
                        self.id, self.registered_processes
                    ));
                }
                RegistryEvent::UPDATE_REGISTERED_PROCESSES {
                    registered_processes,
                } => {
                    self.registered_processes = registered_processes;
                    log(&format!(
                        "Updating registered processes: {:?}",
                        self.registered_processes
                    ));
                }
            }
        }
    }

    fn send_heartbeat(addr: String) {
        log("Sending heartbeat to registry...");

        if Process::process_is_alive(addr) {
            log("Registry is alive");
        } else {
            log("Registry is dead, exiting");
            exit(1);
        }
    }
}

fn handle_buffer(buffer: [u8; 1000], data_size: usize) -> Option<RegistryEvent> {
    RegistryEvent::parse_bytes(&buffer[..data_size])
}

fn log(str: &str) {
    println!("[Process] {}", str);
}
