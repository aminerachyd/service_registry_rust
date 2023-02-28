mod events;
mod process;
mod registry;

use std::{collections::HashMap, io::Write, net::TcpStream};

use process::Process;
use registry::Registry;

pub fn start_registry(addr: String) -> std::io::Result<()> {
    Registry::new().run(&addr)
}

pub fn start_process(port: u32, registry_address: String) -> std::io::Result<()> {
    Process::new().run(port, registry_address)
}

trait P2PSend {
    fn send(to_addr: &String, buffer: &[u8]) -> std::io::Result<usize> {
        let mut stream = TcpStream::connect(to_addr).unwrap();

        stream.write(buffer)
    }

    fn process_is_alive(addr: String) -> bool {
        match TcpStream::connect(addr) {
            Ok(_) => true,
            Err(_) => false,
        }
    }
}

trait Broadcast: P2PSend {
    fn broadcast_to_all(processes: &HashMap<u32, String>, buffer: &[u8]) {
        processes.iter().for_each(|(_, addr)| {
            Self::send(&addr, buffer).unwrap();
        })
    }
}
