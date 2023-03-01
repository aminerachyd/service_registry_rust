use std::{
    collections::HashMap,
    io::Read,
    net::{IpAddr, TcpListener},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use crate::{
    events::{ProcessEvent, RegistryEvent},
    Broadcast, P2PSend,
};

type Processes = Arc<Mutex<HashMap<u32, String>>>;
type AMu32 = Arc<Mutex<u32>>;

pub struct Registry {
    processes: Processes,
    last_registered_id: AMu32,
}
impl P2PSend for Registry {}
impl Broadcast for Registry {}

impl Registry {
    pub fn new() -> Self {
        let processes = HashMap::new();
        Registry {
            last_registered_id: Arc::new(Mutex::new(0)),
            processes: Arc::new(Mutex::new(processes)),
        }
    }

    pub fn run(self, addr: &String) -> std::io::Result<()> {
        let listener = TcpListener::bind(addr)?;
        log(&format!("Started registry on {}", addr));

        // Thread to check all processes if alive
        let mut processes = Arc::clone(&self.processes);
        thread::spawn(move || loop {
            thread::sleep(Duration::from_secs(5));
            Registry::send_heartbeat(&mut processes);
        });

        // Thread to broadcast the processes table
        let mut processes = Arc::clone(&self.processes);
        thread::spawn(move || loop {
            thread::sleep(Duration::from_secs(5));
            Registry::broadcast_registered_processes(&mut processes);
        });

        for stream in listener.incoming() {
            let mut buffer = [0u8; 1000];

            let processes = Arc::clone(&self.processes);
            let last_registered_id = Arc::clone(&self.last_registered_id);

            thread::spawn(move || {
                let mut stream = stream.unwrap();
                let data_size = stream.read(&mut buffer).unwrap();
                let peer_addr = stream.peer_addr().unwrap().ip();

                if data_size > 0 {
                    let process_event = handle_buffer(buffer, data_size);

                    let processes = &mut *(processes.lock().unwrap());
                    let last_registered_id = &mut *(last_registered_id.lock().unwrap());

                    Registry::handle_process_event(
                        peer_addr,
                        process_event,
                        processes,
                        last_registered_id,
                    );
                }
            });
        }
        Ok(())
    }

    fn handle_process_event(
        process_addr: IpAddr,
        process_event: Option<ProcessEvent>,
        processes: &mut HashMap<u32, String>,
        last_id: &mut u32,
    ) {
        if process_event.is_none() {
            log("Data is None");
        } else {
            let process_event = process_event.unwrap();

            match process_event {
                ProcessEvent::CONNECT { port } => {
                    log(&format!("Received CONNECT from {}:{}", process_addr, port));
                    Registry::register_process(
                        format!("{}:{}", process_addr, port),
                        processes,
                        last_id,
                    );
                }
            }
        }
    }

    fn register_process(addr: String, processes: &mut HashMap<u32, String>, last_id: &mut u32) {
        let next_id = *last_id + 1;
        processes.insert(next_id, addr.clone());
        *last_id = next_id;
        log(&format!("Registered process {} at id {}", &addr, next_id));

        let registry_event = &RegistryEvent::REGISTERED {
            id: next_id,
            registered_processes: processes.clone(),
        }
        .as_bytes_vec()[..];

        match Registry::send(&addr, registry_event) {
            Ok(_) => {}
            _ => {
                log(&format!("Couldn't reach process {}", &addr));
            }
        };
    }

    fn broadcast_registered_processes(processes: &mut Processes) {
        log("Sending updated table of processes");
        let processes = &*(processes.lock().unwrap());
        let registry_event = &RegistryEvent::UPDATE_REGISTERED_PROCESSES {
            registered_processes: processes.clone(),
        }
        .as_bytes_vec()[..];

        Registry::broadcast_to_all(&processes, registry_event);
    }

    fn send_heartbeat(processes: &mut Processes) {
        log("Sending heartbeat...");
        let mut dead_processes = vec![];

        let processes = &mut processes.lock().unwrap();

        processes.iter().for_each(|(id, addr)| {
            if Registry::process_is_alive(addr.to_owned()) {
                log(&format!("Process at {} is alive", addr));
            } else {
                log(&format!("Process at {} is dead, removing it...", addr));
                dead_processes.push(id.clone());
            }
        });

        dead_processes.iter().for_each(|&id| {
            processes.remove(&id);
        });
    }
}

fn log(str: &str) {
    println!("[Registry] {}", str);
}

fn handle_buffer(buffer: [u8; 1000], data_size: usize) -> Option<ProcessEvent> {
    ProcessEvent::parse_bytes(&buffer[..data_size])
}
