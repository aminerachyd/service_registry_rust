use std::{
    collections::HashMap,
    io::Read,
    net::TcpListener,
    process::exit,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use rand::Rng;

use crate::{
    events::{Event, ProcessEvent, RegistryEvent},
    handle_buffer, P2PSend,
};

type Processes = Arc<Mutex<HashMap<u32, String>>>;

pub struct Process {
    id: u32,
    address: String,
    port: u32,
    registry_address: String,
    registered_processes: Processes,
}

impl P2PSend for Process {}

impl Process {
    pub fn new() -> Self {
        Process {
            id: std::process::id(),
            address: String::from(""),
            port: 8080,
            registry_address: String::from(""),
            registered_processes: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn run(mut self, port: u32, registry_address: String) -> std::io::Result<()> {
        let listener = TcpListener::bind(format!("0.0.0.0:{}", port))?;
        log(&format!("Started process on port {}", port));

        self.address = format!("0.0.0.0:{}", port);
        self.port = port;
        self.registry_address = registry_address;

        self.connect_to_registry(self.registry_address.clone());

        let registry_address_clone = self.registry_address.clone();

        // Periodically check if the registry is still alive
        thread::spawn(move || loop {
            thread::sleep(Duration::from_secs(5));

            Process::send_heartbeat(registry_address_clone.to_owned());
        });

        // FIXME not working
        // Periodically send a message to a random process
        let arc_registered_processes = Arc::clone(&self.registered_processes);
        thread::spawn(move || loop {
            thread::sleep(Duration::from_secs(2));
            let registered_processes = &*arc_registered_processes.lock().unwrap();
            dbg!(registered_processes);

            if registered_processes.len() > 0 {
                Process::send_to_random_process(registered_processes);
            }
        });

        for stream in listener.incoming() {
            let mut buffer = [0u8; 1000];
            let mut stream = stream.unwrap();
            let data_size = stream.read(&mut buffer).unwrap();

            let arc_registered_processes = Arc::clone(&self.registered_processes);
            thread::spawn(move || {
                if data_size > 0 {
                    let event = handle_buffer(buffer, data_size);

                    match event {
                        Some(Event::ProcessEvent(process_event)) => {
                            Process::handle_process_event(Some(process_event));
                        }
                        Some(Event::RegistryEvent(registry_event)) => {
                            Process::handle_registry_event(
                                arc_registered_processes,
                                Some(registry_event),
                            );
                        }
                        None => {}
                        _ => {}
                    };
                }
            });
        }
        Ok(())
    }

    fn connect_to_registry(&self, registry_address: String) {
        log("Connecting to registry...");
        let connect_event = &ProcessEvent::CONNECT_ON_PORT {
            id: self.id.clone(),
            port: self.port.clone(),
        }
        .as_bytes_vec()[..];
        match Process::send(&registry_address, connect_event) {
            Ok(_) => {}
            _ => {
                log("Couldn't reach registry, exiting");
                exit(1);
            }
        }
    }

    fn handle_registry_event(
        registered_processes: Processes,
        registry_event: Option<RegistryEvent>,
    ) {
        if registry_event.is_none() {
            log("Data is None");
        } else {
            let registry_event = registry_event.unwrap();

            match registry_event {
                RegistryEvent::REGISTERED {
                    registered_processes: update_processes,
                } => {
                    let mut local_registered_processes = &*registered_processes.lock().unwrap();
                    local_registered_processes = &update_processes;
                    log(&format!(
                        "Connected to registry, given id: {}\n Registered processes {:?}",
                        std::process::id(),
                        local_registered_processes
                    ));
                }
                RegistryEvent::UPDATE_REGISTERED_PROCESSES(update_processes) => {
                    let mut local_registered_processes = &*registered_processes.lock().unwrap();
                    local_registered_processes = &update_processes;
                    log(&format!(
                        "Updating registered processes: {:?}",
                        local_registered_processes
                    ));
                }
            }
        }
    }

    fn handle_process_event(process_event: Option<ProcessEvent>) {
        if process_event.is_none() {
            log("Data is None");
        } else {
            let process_event = process_event.unwrap();

            match process_event {
                ProcessEvent::MESSAGE(msg) => {
                    log(&format!("Received message: {}", msg));
                }
                _ => {}
            }
        }
    }

    fn send_heartbeat(addr: String) {
        log("Sending heartbeat to registry...");

        if Process::process_is_alive(addr) {
            log("Registry is alive");
        } else {
            log("Registry is dead, exiting");
            exit(0);
        }
    }

    fn get_process_addr(id: u32, processes: &HashMap<u32, String>) -> Option<String> {
        match processes.get(&id) {
            Some(addr) => Some(addr.to_owned()),
            None => None,
        }
    }

    fn send_to_random_process(processes: &HashMap<u32, String>) {
        let mut rng = rand::thread_rng();
        let process_ids: &Vec<u32> = &processes.iter().map(|(k, _)| k.to_owned()).collect();
        let mut random_index = rng.gen_range((0..process_ids.len()));

        let mut process_id = process_ids.get(random_index).unwrap().to_owned();

        let self_id = std::process::id();
        let message_event =
            &ProcessEvent::MESSAGE(format!("Hello from {}", self_id)).as_bytes_vec()[..];
        match Process::get_process_addr(process_id, processes) {
            Some(addr) => {
                Process::send(&addr, message_event);
            }
            None => {
                log(&format!("Process {} doesn't exist", process_id));
            }
        }
    }
}

fn log(str: &str) {
    println!("[Process {}] {}", std::process::id(), str);
}
