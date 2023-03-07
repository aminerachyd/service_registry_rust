mod algorithms;
mod events;
mod process;
mod registry;

use algorithms::{Broadcast, P2PSend};
use events::Event;
use process::Process;
use registry::Registry;

pub fn start_registry(addr: String) -> std::io::Result<()> {
    Registry::new().run(&addr)
}

pub fn start_process(port: u32, registry_address: String) -> std::io::Result<()> {
    let process = Process::new(port, registry_address.clone())?;
    process.run(port)
}

fn handle_buffer(buffer: [u8; 1000], data_size: usize) -> Option<Event> {
    Event::parse_event_type(&buffer[..data_size])
}
