use std::{io::Read, net::TcpListener};

const BUFFER_SIZE: usize = 1024 * 1024;

fn main() {
    let listener = TcpListener::bind("127.0.0.1:5200").unwrap();

    for stream in listener.incoming() {
        let mut stream = stream.unwrap();
        println!("New connection: {}", stream.peer_addr().unwrap());

        let mut buffer = [0; BUFFER_SIZE];
        let bytes_read = stream.read(&mut buffer).unwrap();

        let text = std::str::from_utf8(&buffer[..bytes_read]).unwrap();
        println!("Received: {:?}", text);
    }
}
