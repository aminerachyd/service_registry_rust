use std::{
    collections::HashMap,
    io::Read,
    net::{IpAddr, TcpListener},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use crate::{
    events::{Event, ProcessEvent, RegistryEvent},
    handle_buffer, Broadcast, P2PSend,
};

type Processes = Arc<Mutex<HashMap<u32, String>>>;

pub struct Registry {
    processes: Processes,
}
impl P2PSend for Registry {}
impl Broadcast for Registry {}

impl Registry {
    pub fn new() -> Self {
        let processes = HashMap::new();
        Registry {
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

            thread::spawn(move || {
                let mut stream = stream.unwrap();
                let data_size = stream.read(&mut buffer).unwrap();
                let peer_addr = stream.peer_addr().unwrap().ip();

                if data_size > 0 {
                    let event = handle_buffer(buffer, data_size);

                    let processes = &mut *(processes.lock().unwrap());

                    match event {
                        Some(Event::ProcessEvent(process_event)) => {
                            Registry::handle_process_event(
                                peer_addr,
                                Some(process_event),
                                processes,
                            );
                        }
                        Some(Event::RegistryEvent(_)) => {
                            log("Another registry running ?!");
                        }
                        None => {}
                    };
                }
            });
        }
        Ok(())
    }

    fn handle_process_event(
        process_addr: IpAddr,
        process_event: Option<ProcessEvent>,
        processes: &mut HashMap<u32, String>,
    ) {
        if process_event.is_none() {
            log("Data is None");
        } else {
            let process_event = process_event.unwrap();

            match process_event {
                ProcessEvent::CONNECT_ON_PORT { id, port } => {
                    log(&format!("Received CONNECT from {}:{}", process_addr, port));
                    Registry::register_process(format!("{}:{}", process_addr, port), processes, id);
                }
                _ => {}
            }
        }
    }

    fn register_process(addr: String, processes: &mut HashMap<u32, String>, process_id: u32) {
        processes.insert(process_id, addr.clone());
        log(&format!(
            "Registered process {} at id {}",
            &addr, process_id
        ));

        let registry_event = &RegistryEvent::REGISTERED {
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
        let processes = &*(processes.lock().unwrap());

        if processes.len() > 0 {
            log("Sending updated table of processes");
            let registry_event =
                &RegistryEvent::UPDATE_REGISTERED_PROCESSES(processes.clone()).as_bytes_vec()[..];

            Registry::broadcast_to_all(&processes, registry_event);
        }
    }

    fn send_heartbeat(processes: &mut Processes) {
        let processes = &mut processes.lock().unwrap();

        if processes.len() > 0 {
            log("Sending heartbeat...");
            let mut dead_processes = vec![];

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
}

fn log(str: &str) {
    println!("[Registry] {}", str);
}
