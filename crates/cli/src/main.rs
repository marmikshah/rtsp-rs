use clap::Parser;
use rtsp::Server;
use std::io;

#[derive(Parser)]
#[command(
    name = "rtsp-server",
    about = "Standalone RTSP server for H.264 streams"
)]
struct Args {
    /// Bind address (host:port)
    #[arg(long, short, default_value = "0.0.0.0:8554")]
    bind: String,
}

fn main() {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    let mut server = Server::new(&args.bind);

    if let Err(e) = server.start() {
        eprintln!("Failed to start server: {}", e);
        return;
    }

    println!("RTSP server on {} — press Enter to stop", args.bind);
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();

    server.stop();
}
