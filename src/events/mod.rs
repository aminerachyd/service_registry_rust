use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub enum Event {
    ProcessEvent(ProcessEvent),
    RegistryEvent(RegistryEvent),
    PaxosAcceptorEvent(PaxosAcceptorEvent),
    PaxosProposerEvent(PaxosProposerEvent),
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ProcessEvent {
    ConnectOnPort { port: u32 },
    Message { from: u32, msg: String },
}

#[derive(Serialize, Deserialize, Debug)]
pub enum RegistryEvent {
    Registered {
        given_id: u32,
        registered_processes: HashMap<u32, String>,
    },
    UpdateRegisteredProcesses(HashMap<u32, String>),
}

#[derive(Serialize, Deserialize, Debug)]
pub enum PaxosProposerEvent {
    Prepare { proposer_id: u32 },
    RequestAccept { proposer_id: u32, value: u32 },
}

#[derive(Serialize, Deserialize, Debug)]
pub enum PaxosAcceptorEvent {
    Promise { proposer_id: u32, value: u32 },
    Accepted { proposer_id: u32, value: u32 },
}

impl Event {
    pub fn parse_event_type(bytes: &[u8]) -> Option<Event> {
        match ProcessEvent::parse_bytes(bytes) {
            Some(process_event) => Some(Event::ProcessEvent(process_event)),
            None => match RegistryEvent::parse_bytes(bytes) {
                Some(registry_event) => Some(Event::RegistryEvent(registry_event)),
                None => match PaxosAcceptorEvent::parse_bytes(bytes) {
                    Some(paxos_acceptor_event) => {
                        Some(Event::PaxosAcceptorEvent(paxos_acceptor_event))
                    }
                    None => match PaxosProposerEvent::parse_bytes(bytes) {
                        Some(paxos_proposer_event) => {
                            Some(Event::PaxosProposerEvent(paxos_proposer_event))
                        }
                        None => None,
                    },
                },
            },
        }
    }
}

impl RegistryEvent {
    pub fn as_bytes_vec(self) -> Vec<u8> {
        self.serialize()
    }

    pub fn parse_bytes(bytes: &[u8]) -> Option<Self> {
        match Self::deserialize(bytes.to_vec()) {
            Ok(event) => Some(event),
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
    pub fn parse_bytes(bytes: &[u8]) -> Option<Self> {
        match Self::deserialize(bytes.to_vec()) {
            Ok(event) => Some(event),
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

impl PaxosAcceptorEvent {
    pub fn as_bytes_vec(self) -> Vec<u8> {
        self.serialize()
    }
    pub fn parse_bytes(bytes: &[u8]) -> Option<Self> {
        match Self::deserialize(bytes.to_vec()) {
            Ok(event) => Some(event),
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

impl PaxosProposerEvent {
    pub fn as_bytes_vec(self) -> Vec<u8> {
        self.serialize()
    }
    pub fn parse_bytes(bytes: &[u8]) -> Option<Self> {
        match Self::deserialize(bytes.to_vec()) {
            Ok(event) => Some(event),
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
