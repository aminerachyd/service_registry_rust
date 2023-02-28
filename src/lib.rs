mod events;
mod process;
mod registry;

use process::Process;
use registry::Registry;

pub fn start_registry(addr: String) -> std::io::Result<()> {
    Registry::new(&addr).run(&addr)
}

pub fn start_process(port: u32, registry_address: String) -> std::io::Result<()> {
    Process::new().run(port, registry_address)
}
