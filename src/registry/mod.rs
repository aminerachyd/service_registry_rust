use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use crate::events::{ProcessEvent, RegistryEvent};

type Processes = Arc<Mutex<HashMap<u32, String>>>;
type AMu32 = Arc<Mutex<u32>>;

pub struct Registry {
    processes: Processes,
    last_registered_id: AMu32,
}

fn log(str: &str) {
    println!("[Registry] {}", str);
}

fn handle_buffer(buffer: [u8; 1000], data_size: usize) -> Option<ProcessEvent> {
    ProcessEvent::parse_bytes(&buffer[..data_size])
}

impl Registry {
    pub fn new(addr: &String) -> Self {
        let mut processes = HashMap::new();
        processes.insert(0, addr.to_owned());
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

        for stream in listener.incoming() {
            let mut buffer = [0u8; 1000];

            let processes = Arc::clone(&self.processes);
            let last_registered_id = Arc::clone(&self.last_registered_id);

            thread::spawn(move || {
                let mut stream = stream.unwrap();
                let data_size = stream.read(&mut buffer).unwrap();

                if data_size > 0 {
                    let process_event = handle_buffer(buffer, data_size);

                    let processes = &mut *(processes.lock().unwrap());
                    let last_registered_id = &mut *(last_registered_id.lock().unwrap());

                    Registry::handle_process_event(process_event, processes, last_registered_id);
                }
            });
        }
        Ok(())
    }

    fn handle_process_event(
        process_event: Option<ProcessEvent>,
        processes: &mut HashMap<u32, String>,
        last_id: &mut u32,
    ) {
        if process_event.is_none() {
            log("Data is None");
        } else {
            let process_event = process_event.unwrap();

            match process_event {
                ProcessEvent::CONNECT { addr } => {
                    log(&format!("Received CONNECT from {}", addr));
                    Registry::register_process(addr, processes, last_id);
                }
            }
        }
    }

    fn register_process(addr: String, processes: &mut HashMap<u32, String>, last_id: &mut u32) {
        let mut stream = TcpStream::connect(&addr).unwrap();
        let next_id = *last_id + 1;
        processes.insert(next_id, addr.clone());
        *last_id = next_id;
        log(&format!("Registered process {} at id {}", &addr, next_id));

        let registry_event = &RegistryEvent::REGISTERED {
            id: next_id,
            registered_processes: processes.clone(),
        }
        .as_bytes_vec()[..];
        stream.write(registry_event).unwrap();
    }

    fn send_heartbeat(processes: &mut Processes) {
        log("Sending heartbeat...");
        let mut dead_processes = vec![];

        let processes = &mut processes.lock().unwrap();

        processes.iter().for_each(|(id, addr)| {
            if Registry::process_is_alive(addr) {
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

    fn process_is_alive(addr: &String) -> bool {
        match TcpStream::connect(addr) {
            Ok(_) => true,
            Err(_) => false,
        }
    }
}
