use std::io::{Read, Write};
use std::net::TcpListener;

mod handler;
mod protocol;

use handler::handle_request;
use protocol::parse_request;

fn main() {
    let listener = TcpListener::bind("0.0.0.0:8554").expect("Failed to bind to port");
    println!("RTSP Server listening on port 8554...");

    for stream in listener.incoming() {
        match stream {
            Ok(mut connection) => {
                println!("Client connected: {}", connection.peer_addr().unwrap());

                let mut buffer = [0u8; 1024];
                match connection.read(&mut buffer) {
                    Ok(bytes_read) => {
                        let raw_request = String::from_utf8_lossy(&buffer[..bytes_read]);

                        match parse_request(&raw_request) {
                            Ok(request) => {
                                let response = handle_request(&request);
                                let response_bytes = response.serialize();

                                println!("<<< Sending:\n{}", response_bytes);

                                if let Err(e) = connection.write_all(response_bytes.as_bytes()) {
                                    println!("Failed to send response: {}", e);
                                }
                            }
                            Err(e) => println!("Parse error: {:?}", e),
                        }
                    }
                    Err(e) => println!("Failed to read: {}", e),
                }
            }
            Err(e) => println!("Connection failed: {}", e),
        }
    }
}
