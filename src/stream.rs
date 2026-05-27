pub struct UtpStream {
    conn_id: u16,
    peer_addr: std::net::SocketAddr,
}

impl UtpStream {
    pub fn peer_addr(&self) -> std::net::SocketAddr { self.peer_addr }
    pub fn conn_id(&self) -> u16 { self.conn_id }


    pub fn new(conn_id: u16, addr: std::net::SocketAddr) -> Self {
        Self {
            conn_id,
            peer_addr: addr,
        }
    }
}
