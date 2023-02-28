use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub enum ProcessEvent {
    CONNECT { addr: String },
}

#[derive(Serialize, Deserialize, Debug)]
pub enum RegistryEvent {
    REGISTERED {
        id: u32,
        registered_processes: HashMap<u32, String>,
    },
}

impl RegistryEvent {
    pub fn as_bytes_vec(self) -> Vec<u8> {
        self.serialize()
    }

    pub fn parse_bytes(bytes: &[u8]) -> Option<RegistryEvent> {
        let str = String::from_utf8(bytes.to_vec()).unwrap();
        println!("{}", str);
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