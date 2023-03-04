use std::{
    collections::HashMap,
    io::{ErrorKind, Write},
    net::{TcpStream, ToSocketAddrs},
    sync::mpsc,
    thread,
    time::Duration,
};

use crate::events::{PaxosAcceptedValue, PaxosAcceptorEvent, PaxosProposerEvent};

pub trait P2PSend {
    const TIMEOUT: Duration = Duration::from_secs(5);

    fn send(to_addr: &String, buffer: &[u8]) -> std::io::Result<usize> {
        let (sender, receiver) = mpsc::channel();

        let addr = to_addr.clone();
        thread::spawn(move || {
            match sender.send(addr.to_socket_addrs().unwrap().next().unwrap()) {
                Ok(_) => {}
                Err(_) => {}
            };
        });

        thread::sleep(Self::TIMEOUT);

        match receiver.try_recv() {
            Ok(addr) => {
                let mut stream = TcpStream::connect_timeout(&addr, Self::TIMEOUT)?;

                stream.write(buffer)
            }
            _ => Err(std::io::ErrorKind::AddrNotAvailable.into()),
        }
    }

    fn process_is_alive(addr: String) -> bool {
        match Self::send(&addr, &[]) {
            Ok(_) => true,
            Err(_) => false,
        }
    }
}

pub trait Broadcast: P2PSend {
    fn broadcast_to_all(processes: &HashMap<u32, String>, buffer: &[u8]) -> std::io::Result<usize> {
        let results: Vec<std::io::Result<usize>> = processes
            .iter()
            .map(|(_, addr)| Self::send(&addr, buffer))
            .collect();

        if results
            .iter()
            .filter(|r| r.is_err())
            .collect::<Vec<_>>()
            .len()
            > 0
        {
            Err(ErrorKind::Other.into())
        } else {
            match results.last().unwrap() {
                Ok(usize) => Ok(*usize),
                Err(e) => match e.kind() {
                    ErrorKind::AddrNotAvailable => Err(ErrorKind::AddrNotAvailable.into()),
                    _ => Err(ErrorKind::Other.into()),
                },
            }
        }
    }
}

pub trait PaxosProposer: Broadcast {
    fn prepare(seq_number: u32, acceptors: &HashMap<u32, String>) -> std::io::Result<usize> {
        let request = &PaxosProposerEvent::Prepare { seq_number }.as_bytes_vec()[..];

        Self::broadcast_to_all(acceptors, request)
    }

    fn request_accept(
        seq_number: u32,
        value: PaxosAcceptedValue,
        acceptors: &HashMap<u32, String>,
    ) -> std::io::Result<usize> {
        let request = &PaxosProposerEvent::RequestAccept { seq_number, value }.as_bytes_vec()[..];

        Self::broadcast_to_all(acceptors, request)
    }
}

pub trait PaxosAcceptor: P2PSend {
    fn promise(
        seq_number: u32,
        value: Option<PaxosAcceptedValue>,
        proposer: String,
    ) -> std::io::Result<usize> {
        let request = &PaxosAcceptorEvent::Promise { seq_number, value }.as_bytes_vec()[..];

        Self::send(&proposer, request)
    }
    fn respond_accept(
        seq_number: u32,
        value: Option<PaxosAcceptedValue>,
        proposer: String,
    ) -> std::io::Result<usize> {
        let request = &PaxosAcceptorEvent::Accepted { seq_number, value }.as_bytes_vec()[..];

        Self::send(&proposer, request)
    }
}
