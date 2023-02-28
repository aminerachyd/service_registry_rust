mod events;
mod process;
mod registry;

use process::Process;
use registry::Registry;

pub fn start_registry(port: u32) -> std::io::Result<()> {
    Registry::new().run(port)
}

pub fn start_process(port: u32) -> std::io::Result<()> {
    Process::new().run(port)
}
