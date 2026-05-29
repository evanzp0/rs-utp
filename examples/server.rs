
use std::{io, net::SocketAddr};

use rs_utp::socket::UtpSocket;

#[tokio::main]
async fn main() -> io::Result<()> {
    let addr: SocketAddr = "0.0.0.0:19000".parse().unwrap();
    let socket = UtpSocket::bind(addr).await?;
    
    println!("Server listening on {}", addr);

    loop {
        match socket.accept().await {
            Ok(stream) => {
                println!("Accepted connection from {}", stream.peer_addr());
                // V0.3 会在这里 spawn 一个任务处理 stream 的读写
            }
            Err(e) => {
                eprintln!("Accept error: {}", e);
            }
        }
    }
}