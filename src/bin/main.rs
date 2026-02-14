use rtsp::server::RtspServer;
use std::io;

fn main() {
    let mut server = RtspServer::new("0.0.0.0:8554");

    if let Err(e) = server.start() {
        eprintln!("Failed to start server: {}", e);
        return;
    }

    println!("Press Enter to stop the server...");
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();

    server.stop();
}
