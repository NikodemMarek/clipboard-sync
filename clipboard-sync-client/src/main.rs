use std::{
    fs::OpenOptions,
    io::{Read, Write},
    net::TcpStream,
};

const BUFFER_SIZE: usize = 1024 * 1024;

fn main() {
    let mut stream = TcpStream::connect("127.0.0.1:5200").unwrap();

    // open clipboard pipe
    let mut fifo = OpenOptions::new()
        .read(true)
        .open("./clipboard.pipe")
        .unwrap();

    let mut prev_buffer = [0u8; BUFFER_SIZE];
    let mut new_buffer = [0u8; BUFFER_SIZE];

    while let Ok(bytes_read @ 1..) = fifo.read(&mut new_buffer) {
        if new_buffer == prev_buffer {
            continue;
        }

        prev_buffer = new_buffer;
        new_buffer = [0u8; BUFFER_SIZE];

        let data = &prev_buffer[..bytes_read];
        let text = std::str::from_utf8(data).unwrap();
        // println!("Clipboard: {}", text);

        stream.write_all(data.clone()).unwrap();
    }

    println!("Done reading.");
}
