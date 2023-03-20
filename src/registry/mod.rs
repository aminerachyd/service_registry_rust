use std::{
    collections::HashMap,
    io::{ErrorKind, Read},
    net::{IpAddr, TcpListener},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use crate::{
    algorithms::{Logger, PaxosProposer},
    events::{
        Event, PaxosAcceptedValue, PaxosAcceptorEvent, PaxosStatus, ProcessEvent, RegistryEvent,
    },
    handle_buffer, Broadcast, P2PSend,
};

type Processes = Arc<Mutex<HashMap<u32, String>>>;
type AMu32 = Arc<Mutex<u32>>;

pub struct Registry {
    last_registered_id: AMu32,
    processes: Processes,
    paxos_seq_number: u32,
    promises_received: AMu32,
    accepted_received: AMu32,
    accepted_values_received: Arc<Mutex<Vec<Option<PaxosAcceptedValue>>>>,
    paxos_status: Arc<Mutex<PaxosStatus>>,
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
            paxos_seq_number: 0,
            promises_received: Arc::new(Mutex::new(0)),
            accepted_received: Arc::new(Mutex::new(0)),
            accepted_values_received: Arc::new(vec![].into()),
            paxos_status: Arc::new(Mutex::new(PaxosStatus::NoConsensus)),
        }
    }

    pub fn run(&self, addr: &String) -> std::io::Result<()> {
        let listener = TcpListener::bind(addr)?;
        self.log(&format!("Started registry on {}", addr));

        thread::scope(|s| {
            // Thread to check all processes if alive and broadcast the processes table
            s.spawn(move || loop {
                thread::sleep(Duration::from_secs(20));

                let paxos_status = self.paxos_status.try_lock();
                if paxos_status.is_ok() {
                    let paxos_status = &mut *paxos_status.unwrap();

                    self.send_heartbeat(paxos_status);
                    match self.broadcast_registered_processes() {
                        Ok(_) => {}
                        Err(_) => {}
                    };
                }
            });

            // Thread to start paxos consensus instance
            s.spawn(move || loop {
                thread::sleep(Duration::from_secs(10));
                let paxos_status = self.paxos_status.try_lock();
                if paxos_status.is_ok() {
                    let paxos_status = &mut *paxos_status.unwrap();
                    match paxos_status {
                        PaxosStatus::NoConsensus => {
                            self.start_consensus_instance(
                                self.paxos_seq_number.clone(),
                                paxos_status,
                            );
                        }
                        paxos_status => {
                            match paxos_status {
                                PaxosStatus::ConsensusReached(value) => {
                                    self.log(&format!(
                                        "#PAXOS# Consensus is reached, value is {:?}",
                                        value
                                    ));
                                    thread::sleep(Duration::from_secs(5));
                                }
                                PaxosStatus::Phase1 => {
                                    self.log("#PAXOS# Phase 1");
                                }
                                PaxosStatus::Phase2 => {
                                    self.log("#PAXOS# Phase 2");
                                }
                                _ => {}
                            }
                            drop(paxos_status)
                        }
                    }
                }
            });

            // Listen for incoming events
            for stream in listener.incoming() {
                let mut buffer = [0u8; 1000];

                s.spawn(move || {
                    let mut stream = stream.unwrap();
                    let data_size = stream.read(&mut buffer).unwrap();
                    let peer_addr = stream.peer_addr().unwrap().ip();

                    if data_size > 0 {
                        let event = handle_buffer(buffer, data_size);

                        match event {
                            Some(event) => match event {
                                Event::ProcessEvent(process_event) => {
                                    self.handle_process_event(peer_addr, process_event);
                                }
                                Event::PaxosAcceptorEvent(acceptor_event) => {
                                    self.handle_acceptor_event(acceptor_event);
                                }
                                Event::RegistryEvent(_) | Event::PaxosProposerEvent(_) => {
                                    self.log("Another registry running ?!");
                                }
                            },
                            None => {}
                        };
                    }
                });
            }
        });
        Ok(())
    }

    fn handle_process_event(&self, process_addr: IpAddr, process_event: ProcessEvent) {
        let processes = &mut *(self.processes.lock().unwrap());
        let last_registered_id = &mut *(self.last_registered_id).lock().unwrap();
        let paxos_status = &mut *(self.paxos_status).lock().unwrap();
        match process_event {
            ProcessEvent::ConnectOnPort { port } => {
                self.log(&format!("Received CONNECT from {}:{}", process_addr, port));
                self.register_process(
                    format!("{}:{}", process_addr, port),
                    processes,
                    last_registered_id,
                    paxos_status,
                );
            }
            ProcessEvent::Message { from, msg } => {
                self.log(&format!("Received message from process {}: {}", from, msg));
            }
        }
    }

    fn handle_acceptor_event(&self, acceptor_event: PaxosAcceptorEvent) {
        let promises_received = &mut *(self.promises_received).lock().unwrap();
        let accepted_received = &mut *(self.accepted_received).lock().unwrap();
        let accepted_values_received = &mut *(self.accepted_values_received).lock().unwrap();
        let paxos_status = &mut *(self.paxos_status).lock().unwrap();
        let acceptors = &mut *self.processes.lock().unwrap();
        let self_seq_number = self.paxos_seq_number;

        match (acceptor_event, &paxos_status) {
            (PaxosAcceptorEvent::Promise { seq_number, value }, PaxosStatus::Phase1) => {
                self.log(&format!(
                    "#PAXOS# Received promise with seq number {} and value: {:?}",
                    seq_number, value
                ));

                let _ = std::mem::replace(promises_received, *promises_received + 1);
                let majority = acceptors.len() / 2 + 1;
                accepted_values_received.push(value);

                // - Broadcast to acceptors, only if we recieved majority of promise
                if *promises_received >= majority as u32 {
                    let av_with_max_sn = accepted_values_received
                        .iter()
                        .filter(|&av| av.is_some())
                        .map(|av| av.as_ref().unwrap())
                        .max_by(|av1, av2| av1.seq_number.cmp(&av2.seq_number));

                    // If some AVs are not None, take the value with the biggest Sn
                    if av_with_max_sn.is_some() {
                        Registry::request_accept(
                            seq_number,
                            av_with_max_sn.unwrap().clone(),
                            acceptors,
                        );
                    } else {
                        // If all AV are None, propose a value
                        let av = PaxosAcceptedValue {
                            seq_number: self_seq_number.clone(),
                            value: rand::Rng::gen_range(&mut rand::thread_rng(), 100..1000),
                        };

                        Registry::request_accept(self_seq_number, av, acceptors);
                    }
                    self.log("#PAXOS# Moving to Phase2");
                    let _ = std::mem::replace(paxos_status, PaxosStatus::Phase2);
                    let _ = std::mem::replace(promises_received, 0);
                    drop(paxos_status);
                    accepted_values_received.clear();
                }
            }
            (PaxosAcceptorEvent::Accepted { seq_number, value }, PaxosStatus::Phase2) => {
                self.log(&format!(
                    "#PAXOS# Received accepted with seq number {} and value: {:?}",
                    seq_number, value
                ));

                let _ = std::mem::replace(accepted_received, *accepted_received + 1);
                let majority = acceptors.len() / 2 + 1;
                if *accepted_received >= majority as u32 {
                    self.log(&format!(
                        "#PAXOS# Consensus reached with value {:?}",
                        value.unwrap()
                    ));
                    let _ = std::mem::replace(
                        paxos_status,
                        PaxosStatus::ConsensusReached(value.unwrap()),
                    );
                    drop(paxos_status);

                    let _ = std::mem::replace(accepted_received, 0);
                }
            }
            (PaxosAcceptorEvent::KO, _) => {
                self.log("#PAXOS# Received KO");
            }
            (_, PaxosStatus::Phase1) => {
                // Other message than Promise
                self.log("#PAXOS# Wrong event on phase1");
            }
            (_, PaxosStatus::Phase2) => {
                // Other message than Accepted
                self.log("#PAXOS# Wrong event on phase2");
            }
            (_, _) => {
                self.log("#PAXOS# Unknown");
            }
        }
    }

    fn register_process(
        &self,
        addr: String,
        processes: &mut HashMap<u32, String>,
        last_registered_id: &mut u32,
        paxos_status: &mut PaxosStatus,
    ) {
        let next_process_id = *last_registered_id + 1;

        processes.insert(next_process_id, addr.clone());
        self.log(&format!(
            "Registered process {} at id {}",
            &addr, next_process_id
        ));

        let _ = std::mem::replace(paxos_status, PaxosStatus::NoConsensus);
        drop(paxos_status);

        *last_registered_id = next_process_id;

        let registry_event = &RegistryEvent::Registered {
            given_id: *last_registered_id,
            registered_processes: processes.clone(),
        }
        .as_bytes_vec()[..];

        match Registry::send(&addr, registry_event) {
            Ok(_) => {}
            _ => {
                self.log(&format!("Couldn't reach process {}", &addr));
            }
        };
    }

    fn broadcast_registered_processes(&self) -> std::io::Result<usize> {
        let processes = self.processes.try_lock();
        if processes.is_ok() {
            let processes = &mut processes.unwrap();
            if processes.len() > 0 {
                self.log("Sending updated table of processes");
                let registry_event =
                    &RegistryEvent::UpdateRegisteredProcesses(processes.clone()).as_bytes_vec()[..];

                Registry::broadcast_to_all(&processes, registry_event)
            } else {
                drop(processes);
                Err(ErrorKind::Other.into())
            }
        } else {
            Err(ErrorKind::Other.into())
        }
    }

    fn send_heartbeat(&self, paxos_status: &mut PaxosStatus) {
        let processes = self.processes.try_lock();
        if processes.is_ok() {
            let processes = &mut processes.unwrap();

            if processes.len() > 0 {
                self.log("Sending heartbeat...");
                let mut dead_processes = vec![];

                processes.iter().for_each(|(id, addr)| {
                    if Registry::process_is_alive(addr.to_owned()) {
                        self.log(&format!("Process at {} is alive", addr));
                    } else {
                        self.log(&format!("Process at {} is dead, removing it...", addr));
                        dead_processes.push(id.clone());
                    }
                });

                if !dead_processes.is_empty() {
                    dead_processes.iter().for_each(|&id| {
                        &processes.remove(&id);
                    });
                    let _ = std::mem::replace(paxos_status, PaxosStatus::NoConsensus);
                }
            }
        }
    }

    fn start_consensus_instance(&self, seq_number: u32, paxos_status: &mut PaxosStatus) {
        let processes = self.processes.try_lock();
        if processes.is_ok() {
            let processes = &processes.unwrap();
            if processes.len() > 2 {
                self.log("#PAXOS# Starting consensus instance...");
                let _ = std::mem::replace(paxos_status, PaxosStatus::Phase1);
                Registry::prepare(seq_number, processes);
            } else {
                self.log("#PAXOS# Not enough alive processes to start a consensus instance");
                drop(processes);
            }
        }
    }
}

impl Logger for Registry {
    fn what_is_self(&self) -> String {
        "Registry".to_string()
    }
    fn what_is_id(&self) -> Option<u32> {
        None
    }
}
