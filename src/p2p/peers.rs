use std::collections::HashMap;
use std::net::SocketAddr;
use mio::{Token, Interest, Registry};
use mio::net::TcpStream;
use crate::p2p::{Peer, State, Message};
use crate::p2p::network::LISTEN_PORT;
use crate::p2p::network::next;
use rand::random;

pub struct Peers {
    peers: HashMap<Token, Peer>,
    new_peers: Vec<SocketAddr>
}

const PING_PERIOD: u64 = 30;

impl Peers {
    pub fn new() -> Self {
        Peers { peers: HashMap::new(), new_peers: Vec::new() }
    }

    pub fn add_peer(&mut self, token: Token, peer: Peer) {
        self.peers.insert(token, peer);
    }

    pub fn get_peer(&self, token: &Token) -> Option<&Peer> {
        self.peers.get(token)
    }

    pub fn get_mut_peer(&mut self, token: &Token) -> Option<&mut Peer> {
        self.peers.get_mut(token)
    }

    pub fn remove_peer(&mut self, token: &Token) -> Option<Peer> {
        self.peers.remove(token)
    }

    pub fn add_peers_from_exchange(&mut self, peers: Vec<String>) {
        println!("Got peers: {:?}", &peers);
        // TODO make it return error if these peers are wrong and seem like an attack
        for peer in peers.iter() {
            let addr: SocketAddr = peer.parse().expect(&format!("Error parsing peer {}", peer));
            if addr.ip().is_loopback() {
                continue; // Return error in future
            }
            let mut found = false;
            for (_token, p) in self.peers.iter() {
                if p.equals(&addr) {
                    found = true;
                    break;
                }
            }
            if found {
                continue;
            }
            self.new_peers.push(addr);
        }
    }

    pub fn get_peers_for_exchange(&self, peer_address: &SocketAddr) -> Vec<String> {
        let mut result: Vec<String> = Vec::new();
        for (_, peer) in self.peers.iter() {
            if peer.equals(peer_address) {
                continue;
            }
            if peer.is_public() {
                result.push(SocketAddr::new(peer.get_addr().ip(), LISTEN_PORT).to_string());
            }
        }
        result
    }

    pub fn skip_peer_connection(&self, addr: &SocketAddr) -> bool {
        for (_, peer) in self.peers.iter() {
            if peer.equals(addr) && (!peer.is_public() || peer.active() || peer.disabled()) {
                return true;
            }
        }
        false
    }

    pub fn send_pings(&mut self, registry: &Registry, height: u64) {
        for (token, mut peer) in self.peers.iter_mut() {
            match peer.get_state() {
                State::Idle { from } => {
                    if from.elapsed().as_secs() >= PING_PERIOD {
                        // Sometimes we check for new peers instead of pinging
                        let random: u8 = random();
                        let message = if random < 16 {
                            Message::GetPeers
                        } else {
                            Message::ping(height)
                        };

                        peer.set_state(State::message(message));
                        let mut stream = peer.get_stream();
                        registry.reregister(stream, token.clone(), Interest::WRITABLE).unwrap();
                    }
                }
                _ => {}
            }
        }
    }

    pub fn connect_new_peers(&mut self, registry: &Registry, unique_token: &mut Token) {
        if self.new_peers.is_empty() {
            return;
        }
        for addr in self.new_peers.iter() {
            match TcpStream::connect(addr.clone()) {
                Ok(mut stream) => {
                    println!("Created connection to peer {}", &addr);
                    let token = next(unique_token);
                    registry.register(&mut stream, token, Interest::WRITABLE).unwrap();
                    let mut peer = Peer::new(addr.clone(), stream, State::Connecting, false);
                    peer.set_public(true);
                    self.peers.insert(token, peer);
                }
                Err(e) => {
                    println!("Error connecting to peer {}: {}", &addr, e);
                }
            }
        }
        self.new_peers.clear();
    }
}