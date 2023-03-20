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
    algorithms::{Logger, PaxosAcceptor},
    events::{Event, PaxosAcceptedValue, PaxosProposerEvent, ProcessEvent, RegistryEvent},
    handle_buffer, Broadcast, P2PSend,
};

type Processes = Arc<Mutex<HashMap<u32, String>>>;
type AMu32 = Arc<Mutex<u32>>;

#[derive(Debug)]
pub struct Process {
    id: AMu32,
    port: u32,
    registry_address: String,
    registered_processes: Processes,
    paxos_sn: AMu32,
    paxos_av: Arc<Mutex<Option<PaxosAcceptedValue>>>,
}

impl P2PSend for Process {}
impl Broadcast for Process {}
impl PaxosAcceptor for Process {}

impl Process {
    pub fn new(port: u32, registry_address: String) -> std::io::Result<Self> {
        let _ = TcpListener::bind(format!("0.0.0.0:{}", port))?;
        Ok(Process {
            id: Arc::new(Mutex::new(0)),
            port: port,
            registry_address: registry_address,
            registered_processes: Arc::new(Mutex::new(HashMap::new())),
            paxos_sn: Arc::new(Mutex::new(0)),
            paxos_av: Arc::new(Mutex::new(None)),
        })
    }

    pub fn run(&self) -> std::io::Result<()> {
        let listener = TcpListener::bind(format!("0.0.0.0:{}", self.port))?;
        self.log(&format!("Started process on port {}", self.port));

        self.connect_to_registry(self.registry_address.clone());

        thread::scope(|s| {
            // Periodically check if the registry is still alive
            s.spawn(move || loop {
                thread::sleep(Duration::from_secs(5));

                self.send_heartbeat_to_registry();
            });

            // Periodically send a message to a random process
            s.spawn(move || loop {
                thread::sleep(Duration::from_secs(20));

                match self.send_to_random_process() {
                    Ok(_) => {}
                    Err(_) => {}
                }
            });

            // Periodically broadcast a message to all processes
            s.spawn(move || loop {
                thread::sleep(Duration::from_secs(5));

                let registered_processes = self.registered_processes.try_lock();
                if registered_processes.is_ok() {
                    match self.broadcast_to_processes(&*registered_processes.unwrap()) {
                        Ok(_) => {}
                        Err(_) => {}
                    }
                }
            });

            // Listen for incoming events
            for stream in listener.incoming() {
                let mut buffer = [0u8; 1000];

                s.spawn(move || {
                    let mut stream = stream.unwrap();
                    let data_size = stream.read(&mut buffer).unwrap();

                    let proposer_address = self.registry_address.clone();
                    if data_size > 0 {
                        let event = handle_buffer(buffer, data_size);

                        match event {
                            Some(event) => match event {
                                Event::ProcessEvent(process_event) => {
                                    self.handle_process_event(process_event);
                                }
                                Event::RegistryEvent(registry_event) => {
                                    self.handle_registry_event(registry_event);
                                }
                                Event::PaxosProposerEvent(proposer_event) => {
                                    self.handle_proposer_event(proposer_event, proposer_address);
                                }
                                Event::PaxosAcceptorEvent(_) => {}
                            },
                            None => {
                                self.log("Received something else");
                            }
                        };
                    }
                });
            }
        });
        Ok(())
    }

    fn connect_to_registry(&self, registry_address: String) {
        self.log("Connecting to registry...");
        let connect_event = &ProcessEvent::ConnectOnPort {
            port: self.port.clone(),
        }
        .as_bytes_vec()[..];
        match Process::send(&registry_address, connect_event) {
            Ok(_) => {}
            _ => {
                self.log("Couldn't reach registry, exiting");
                exit(1);
            }
        }
    }

    fn handle_registry_event(&self, registry_event: RegistryEvent) {
        let self_id = &self.id;
        let registered_processes = &self.registered_processes;

        match registry_event {
            RegistryEvent::Registered {
                given_id,
                registered_processes: update_processes,
            } => {
                let local_registered_processes = registered_processes.try_lock();
                let local_self_id = self_id.lock();
                if local_registered_processes.is_ok() && local_self_id.is_ok() {
                    let local_registered_processes = &mut *local_registered_processes.unwrap();

                    let _ = std::mem::replace(local_registered_processes, update_processes);
                    let _ = std::mem::replace(&mut *local_self_id.unwrap(), given_id);

                    self.log(&format!(
                        "Connected to registry, given id: {}\n Registered processes {:?}",
                        given_id, local_registered_processes
                    ));
                }
            }
            RegistryEvent::UpdateRegisteredProcesses(update_processes) => {
                let local_registered_processes = registered_processes.try_lock();
                if local_registered_processes.is_ok() {
                    let local_registered_processes = &mut *local_registered_processes.unwrap();

                    let _ = std::mem::replace(local_registered_processes, update_processes);

                    self.log(&format!(
                        "Updating registered processes: {:?}",
                        local_registered_processes
                    ));
                }
            }
        }
    }

    fn handle_process_event(&self, process_event: ProcessEvent) {
        match process_event {
            ProcessEvent::Message { from, msg } => {
                self.log(&format!("Received from {}: {}", from, msg));
            }
            _ => {}
        }
    }

    // FIXME fix self.logic of algorithm
    // - Has to keep:
    //      - Sn = sequence number to which the acceptor responded with a promise
    //      - AV = (Sn,V) last couple that the acceptor accepted
    // Init : Sn = 0; AV=None
    fn handle_proposer_event(&self, proposer_event: PaxosProposerEvent, proposer_address: String) {
        let local_seq_number = &self.paxos_sn;
        let paxos_accepted_value = &self.paxos_av;
        match proposer_event {
            PaxosProposerEvent::Prepare { seq_number } => {
                self.log(&format!(
                    "#PAXOS# Received prepare with seq number {}",
                    seq_number
                ));
                let local_seq_number = &mut *local_seq_number.lock().unwrap();

                // Promise only if seq_number > Sn
                if seq_number >= *local_seq_number {
                    let paxos_accepted_value = &*paxos_accepted_value.lock().unwrap();
                    Process::promise(seq_number, *paxos_accepted_value, proposer_address);
                    // Update Sn
                    let _ = std::mem::replace(local_seq_number, seq_number);
                } else {
                    Process::no_promise(proposer_address);
                }
            }
            PaxosProposerEvent::RequestAccept { seq_number, value } => {
                self.log(&format!(
                    "#PAXOS# Received accept with seq number {} and value {:?}",
                    seq_number, value
                ));
                let local_seq_number = &mut *local_seq_number.lock().unwrap();

                // Accept only if seq_number >= Sn
                if seq_number >= *local_seq_number {
                    let paxos_accepted_value = &mut *paxos_accepted_value.lock().unwrap();
                    let _ = std::mem::replace(paxos_accepted_value, Some(value));

                    Process::respond_accept(seq_number, *paxos_accepted_value, proposer_address);
                } else {
                    Process::no_promise(proposer_address);
                }
            }
        }
    }

    fn send_heartbeat_to_registry(&self) {
        self.log("Sending heartbeat to registry...");

        if Process::process_is_alive(self.registry_address.to_owned()) {
            self.log("Registry is alive");
        } else {
            self.log("Registry is dead, exiting");
            exit(0);
        }
    }

    fn get_process_addr(id: u32, processes: &HashMap<u32, String>) -> Option<String> {
        match processes.get(&id) {
            Some(addr) => Some(addr.to_owned()),
            None => None,
        }
    }

    fn send_to_random_process(&self) -> std::io::Result<usize> {
        let processes = self.registered_processes.try_lock();
        let self_id = self.id.try_lock();
        if processes.is_ok() && self_id.is_ok() {
            let processes = &mut *processes.unwrap();
            let self_id = *&self_id.unwrap().to_owned();

            if processes.len() > 1 {
                let mut rng = rand::thread_rng();
                let process_ids: &Vec<u32> = &processes.iter().map(|(k, _)| k.to_owned()).collect();
                let mut random_index = rng.gen_range(0..process_ids.len());

                let mut process_id = process_ids.get(random_index).unwrap().to_owned();

                while process_id == self_id {
                    random_index = rng.gen_range(0..process_ids.len());
                    process_id = process_ids.get(random_index).unwrap().to_owned();
                }

                let message_event = &ProcessEvent::Message {
                    from: self_id,
                    msg: "P2P message".to_owned(),
                }
                .as_bytes_vec()[..];

                match Process::get_process_addr(process_id, processes) {
                    Some(addr) => Process::send(&addr, message_event),
                    None => Err(std::io::ErrorKind::AddrNotAvailable.into()),
                }
            } else {
                self.log("Not enough registered processes to send a message");
                Err(std::io::ErrorKind::Other.into())
            }
        } else {
            // Couldn't lock processes and self_id
            Err(std::io::ErrorKind::Other.into())
        }
    }

    fn broadcast_to_processes(&self, processes: &HashMap<u32, String>) -> std::io::Result<usize> {
        if processes.len() > 1 {
            let self_id = self.id.try_lock();
            if self_id.is_ok() {
                let self_id = &*self_id.unwrap();
                let processes: HashMap<u32, String> = processes
                    .iter()
                    .filter(|(&k, _)| k != *self_id)
                    .map(|(k, v)| (k.to_owned(), v.to_owned()))
                    .collect();

                let broadcast_message = &ProcessEvent::Message {
                    from: *self_id,
                    msg: "Broadcast message".to_owned(),
                }
                .as_bytes_vec()[..];
                Process::broadcast_to_all(&processes, broadcast_message)
            } else {
                // Couldn't lock self_id
                Err(std::io::ErrorKind::Other.into())
            }
        } else {
            self.log("Not enough registered processes to broadcast a message");
            Err(std::io::ErrorKind::Other.into())
        }
    }
}

impl Logger for Process {
    fn what_is_self(&self) -> String {
        "Process".to_string()
    }

    fn what_is_id(&self) -> Option<u32> {
        let id = self.id.try_lock();
        if id.is_ok() {
            Some(*id.unwrap())
        } else {
            None
        }
    }
}
