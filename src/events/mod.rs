use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub enum Event {
    ProcessEvent(ProcessEvent),
    RegistryEvent(RegistryEvent),
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ProcessEvent {
    CONNECT_ON_PORT { id: u32, port: u32 },
    MESSAGE { from: u32, msg: String },
}

#[derive(Serialize, Deserialize, Debug)]
pub enum RegistryEvent {
    REGISTERED {
        registered_processes: HashMap<u32, String>,
    },
    UPDATE_REGISTERED_PROCESSES(HashMap<u32, String>),
}

impl Event {
    pub fn parse_event_type(bytes: &[u8]) -> Option<Event> {
        match ProcessEvent::parse_bytes(bytes) {
            Some(process_event) => Some(Event::ProcessEvent(process_event)),
            None => match RegistryEvent::parse_bytes(bytes) {
                Some(registry_event) => Some(Event::RegistryEvent(registry_event)),
                None => None,
            },
        }
    }
}

impl RegistryEvent {
    pub fn as_bytes_vec(self) -> Vec<u8> {
        self.serialize()
    }

    pub fn parse_bytes(bytes: &[u8]) -> Option<RegistryEvent> {
        match RegistryEvent::deserialize(bytes.to_vec()) {
            Ok(registry_event) => Some(registry_event),
            Err(_) => None,
        }
    }

    fn serialize(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap()
    }

    fn deserialize(bytes: Vec<u8>) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(&bytes[..])
    }
}

impl ProcessEvent {
    pub fn as_bytes_vec(self) -> Vec<u8> {
        self.serialize()
    }
    pub fn parse_bytes(bytes: &[u8]) -> Option<ProcessEvent> {
        match ProcessEvent::deserialize(bytes.to_vec()) {
            Ok(process_event) => Some(process_event),
            Err(_) => None,
        }
    }

    fn serialize(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap()
    }

    fn deserialize(bytes: Vec<u8>) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(&bytes[..])
    }
}
