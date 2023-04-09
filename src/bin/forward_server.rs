use std::sync::Arc;

use dashmap::DashMap;
use tokio::net::TcpStream;
use uuid::Uuid;

pub struct Server {
    /// Concurrent map of IDs to incoming connections.
    conns: Arc<DashMap<Uuid, TcpStream>>,
}
