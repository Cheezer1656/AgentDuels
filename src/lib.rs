use std::net::{IpAddr, Ipv4Addr, SocketAddr};

pub mod client;

pub const SERVER_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8081);