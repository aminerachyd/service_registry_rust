use std::{
    collections::HashMap,
    io::{ErrorKind, Read},
    net::{IpAddr, TcpListener},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use crate::{
    algorithms::PaxosProposer,
    events::{Event, PaxosAcceptorEvent, ProcessEvent, RegistryEvent},
    handle_buffer, Broadcast, P2PSend,
};

type Processes = Arc<Mutex<HashMap<u32, String>>>;
type AMu32 = Arc<Mutex<u32>>;

pub struct Registry {
    last_registered_id: AMu32,
    processes: Processes,
}
impl P2PSend for Registry {}
impl Broadcast for Registry {}
impl PaxosProposer for Registry {}

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
            match Registry::broadcast_registered_processes(&mut processes) {
                Ok(_) => {}
                Err(_) => {}
            };
        });

        // Thread to start paxos consensus instance
        let processes = Arc::clone(&self.processes);
        thread::spawn(move || loop {
            thread::sleep(Duration::from_secs(10));
            Registry::start_consensus_instance(&processes);
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
                    let event = handle_buffer(buffer, data_size);

                    match event {
                        Some(event) => match event {
                            Event::ProcessEvent(process_event) => {
                                let processes = &mut *(processes.lock().unwrap());
                                let last_registered_id = &mut *last_registered_id.lock().unwrap();

                                Registry::handle_process_event(
                                    peer_addr,
                                    process_event,
                                    processes,
                                    last_registered_id,
                                );
                            }
                            Event::PaxosAcceptorEvent(acceptor_event) => {
                                Registry::handle_acceptor_event(acceptor_event);
                            }
                            Event::RegistryEvent(_) | Event::PaxosProposerEvent(_) => {
                                log("Another registry running ?!");
                            }
                        },
                        None => {}
                    };
                }
            });
        }
        Ok(())
    }

    fn handle_process_event(
        process_addr: IpAddr,
        process_event: ProcessEvent,
        processes: &mut HashMap<u32, String>,
        last_registered_id: &mut u32,
    ) {
        match process_event {
            ProcessEvent::ConnectOnPort { port } => {
                log(&format!("Received CONNECT from {}:{}", process_addr, port));
                Registry::register_process(
                    format!("{}:{}", process_addr, port),
                    processes,
                    last_registered_id,
                );
            }
            ProcessEvent::Message { from, msg } => {
                log(&format!("Received message from process {}: {}", from, msg));
            }
        }
    }

    // TODO
    fn handle_acceptor_event(acceptor_event: PaxosAcceptorEvent) {
        log(&format!(
            "#PAXOS# Received event from acceptor: {:?}",
            acceptor_event
        ));
    }

    fn register_process(
        addr: String,
        processes: &mut HashMap<u32, String>,
        last_registered_id: &mut u32,
    ) {
        let next_process_id = *last_registered_id + 1;

        processes.insert(next_process_id, addr.clone());
        log(&format!(
            "Registered process {} at id {}",
            &addr, next_process_id
        ));

        *last_registered_id = next_process_id;

        let registry_event = &RegistryEvent::Registered {
            given_id: *last_registered_id,
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

    fn broadcast_registered_processes(processes: &mut Processes) -> std::io::Result<usize> {
        let processes = &*(processes.lock().unwrap());

        if processes.len() > 0 {
            log("Sending updated table of processes");
            let registry_event =
                &RegistryEvent::UpdateRegisteredProcesses(processes.clone()).as_bytes_vec()[..];

            Registry::broadcast_to_all(&processes, registry_event)
        } else {
            Err(ErrorKind::Other.into())
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

    fn start_consensus_instance(processes: &Processes) {
        let processes = &processes.lock().unwrap();
        if processes.len() > 2 {
            log("Starting consensus instance...");
            Registry::prepare(0, processes);
        } else {
            log("Not enough alive processes to start a consensus instance");
        }
    }
}

fn log(str: &str) {
    println!("[Registry] {}", str);
}
