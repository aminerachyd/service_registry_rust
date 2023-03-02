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
    handle_buffer, Broadcast, P2PSend,
};

type Processes = Arc<Mutex<HashMap<u32, String>>>;
type AMu32 = Arc<Mutex<u32>>;

#[derive(Debug)]
pub struct Process {
    id: AMu32,
    address: String,
    port: u32,
    registry_address: String,
    registered_processes: Processes,
}

impl P2PSend for Process {}
impl Broadcast for Process {}

impl Process {
    pub fn new() -> Self {
        Process {
            id: Arc::new(Mutex::new(0)),
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

        // Periodically send a message to a random process
        let arc_registered_processes = Arc::clone(&mut self.registered_processes);
        let arc_self_id = Arc::clone(&self.id);
        thread::spawn(move || loop {
            thread::sleep(Duration::from_secs(20));

            let registered_processes = &*arc_registered_processes.lock().unwrap();
            let self_id = &*arc_self_id.lock().unwrap();

            Process::send_to_random_process(self_id.to_owned(), registered_processes);
        });

        // Periodically broadcast a message to all processes
        let arc_registered_processes = Arc::clone(&mut self.registered_processes);
        let arc_self_id = Arc::clone(&self.id);
        thread::spawn(move || loop {
            thread::sleep(Duration::from_secs(5));

            let registered_processes = &*arc_registered_processes.lock().unwrap();
            let self_id = &*arc_self_id.lock().unwrap();

            Process::broadcast_to_processes(self_id.to_owned(), registered_processes);
        });

        for stream in listener.incoming() {
            let mut buffer = [0u8; 1000];
            let mut stream = stream.unwrap();
            let data_size = stream.read(&mut buffer).unwrap();

            let arc_registered_processes = Arc::clone(&mut self.registered_processes);
            let arc_self_id = Arc::clone(&self.id);
            thread::spawn(move || {
                if data_size > 0 {
                    let event = handle_buffer(buffer, data_size);

                    match event {
                        Some(Event::ProcessEvent(process_event)) => {
                            Process::handle_process_event(Some(process_event));
                        }
                        Some(Event::RegistryEvent(registry_event)) => {
                            Process::handle_registry_event(
                                arc_self_id,
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
        self_id: AMu32,
        registered_processes: Processes,
        registry_event: Option<RegistryEvent>,
    ) {
        if registry_event.is_none() {
            log("Data is None");
        } else {
            let registry_event = registry_event.unwrap();

            match registry_event {
                RegistryEvent::REGISTERED {
                    given_id,
                    registered_processes: update_processes,
                } => {
                    let local_registered_processes = &mut *registered_processes.lock().unwrap();
                    let local_self_id = &mut *self_id.lock().unwrap();

                    std::mem::replace(local_registered_processes, update_processes);
                    std::mem::replace(local_self_id, given_id);

                    log(&format!(
                        "Connected to registry, given id: {}\n Registered processes {:?}",
                        given_id, local_registered_processes
                    ));
                }
                RegistryEvent::UPDATE_REGISTERED_PROCESSES(update_processes) => {
                    let local_registered_processes = &mut *registered_processes.lock().unwrap();
                    std::mem::replace(local_registered_processes, update_processes);
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
                ProcessEvent::MESSAGE { from, msg } => {
                    log(&format!("Received from {}: {}", from, msg));
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

    fn send_to_random_process(self_id: u32, processes: &HashMap<u32, String>) {
        if processes.len() > 1 {
            let mut rng = rand::thread_rng();
            let process_ids: &Vec<u32> = &processes.iter().map(|(k, _)| k.to_owned()).collect();
            let mut random_index = rng.gen_range((0..process_ids.len()));

            let mut process_id = process_ids.get(random_index).unwrap().to_owned();

            while process_id == self_id {
                random_index = rng.gen_range((0..process_ids.len()));
                process_id = process_ids.get(random_index).unwrap().to_owned();
            }

            let message_event = &ProcessEvent::MESSAGE {
                from: self_id,
                msg: "P2P message".to_owned(),
            }
            .as_bytes_vec()[..];

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

    fn broadcast_to_processes(self_id: u32, processes: &HashMap<u32, String>) {
        if processes.len() > 1 {
            let processes: HashMap<u32, String> = processes
                .iter()
                .filter(|(&k, _)| k != self_id)
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
                .collect();

            let broadcast_message = &ProcessEvent::MESSAGE {
                from: self_id,
                msg: "Broadcast message".to_owned(),
            }
            .as_bytes_vec()[..];
            Process::broadcast_to_all(&processes, broadcast_message);
        }
    }
}

fn log(str: &str) {
    println!("[Process] {}", str);
}
