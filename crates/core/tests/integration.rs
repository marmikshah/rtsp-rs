//! Integration test: full RTSP handshake OPTIONS → DESCRIBE → SETUP → PLAY.
//!
//! Starts the server on a fixed port, connects with a TCP client, and
//! verifies each response.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use rtsp::Server;

fn rtsp_request(stream: &mut TcpStream, request: &str) -> std::io::Result<String> {
    stream.write_all(request.as_bytes())?;
    stream.flush()?;

    let mut reader = BufReader::new(stream);
    let mut response = String::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        response.push_str(&line);
        if line == "\r\n" || line == "\n" {
            break;
        }
    }

    // Parse Content-Length and read body if present
    if let Some(len) = response
        .lines()
        .find(|l| l.to_lowercase().starts_with("content-length:"))
        .and_then(|l| l.split(':').nth(1))
        .and_then(|v| v.trim().parse::<usize>().ok())
    {
        if len > 0 {
            let mut body = vec![0u8; len];
            reader.read_exact(&mut body)?;
            response.push_str(&String::from_utf8_lossy(&body));
        }
    }

    Ok(response)
}

/// Fixed port for integration test. bind_addr must be explicit (no port 0).
const TEST_BIND: &str = "127.0.0.1:18554";

#[test]
fn full_handshake_options_describe_setup_play() {
    let mut server = Server::new(TEST_BIND);
    server.start().expect("server start");

    let addr = TEST_BIND.to_socket_addrs().unwrap().next().unwrap();
    let mut stream =
        TcpStream::connect_timeout(&addr, Duration::from_secs(2)).expect("connect to server");
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .unwrap();

    let base_uri = "rtsp://127.0.0.1:18554/stream".to_string();

    // OPTIONS
    let opt_req = format!("OPTIONS {} RTSP/1.0\r\nCSeq: 1\r\n\r\n", base_uri);
    let opt_resp = rtsp_request(&mut stream, &opt_req).expect("OPTIONS response");
    assert!(
        opt_resp.starts_with("RTSP/1.0 200 OK"),
        "OPTIONS: expected 200 OK, got: {}",
        opt_resp.lines().next().unwrap_or("")
    );
    assert!(
        opt_resp.contains("Public:"),
        "OPTIONS: missing Public header"
    );

    // DESCRIBE
    let desc_req = format!(
        "DESCRIBE {} RTSP/1.0\r\nCSeq: 2\r\nAccept: application/sdp\r\n\r\n",
        base_uri
    );
    let desc_resp = rtsp_request(&mut stream, &desc_req).expect("DESCRIBE response");
    assert!(
        desc_resp.starts_with("RTSP/1.0 200 OK"),
        "DESCRIBE: expected 200 OK, got: {}",
        desc_resp.lines().next().unwrap_or("")
    );
    assert!(
        desc_resp.contains("Content-Type: application/sdp"),
        "DESCRIBE: missing Content-Type application/sdp"
    );
    assert!(desc_resp.contains("v=0"), "DESCRIBE: SDP body missing v=0");
    assert!(
        desc_resp.contains("m=video"),
        "DESCRIBE: SDP body missing m=video"
    );
    assert!(
        desc_resp.contains("a=rtpmap:96 H264/90000"),
        "DESCRIBE: SDP missing H264 rtpmap"
    );
    assert!(
        desc_resp.contains("a=fmtp:96 packetization-mode=1"),
        "DESCRIBE: SDP missing fmtp packetization-mode=1"
    );

    // SETUP (track1)
    let setup_uri = format!("{}/track1", base_uri);
    let setup_req = format!(
        "SETUP {} RTSP/1.0\r\nCSeq: 3\r\nTransport: RTP/AVP;unicast;client_port=5000-5001\r\n\r\n",
        setup_uri
    );
    let setup_resp = rtsp_request(&mut stream, &setup_req).expect("SETUP response");
    assert!(
        setup_resp.starts_with("RTSP/1.0 200 OK"),
        "SETUP: expected 200 OK, got: {}",
        setup_resp.lines().next().unwrap_or("")
    );
    assert!(
        setup_resp.contains("Session:"),
        "SETUP: missing Session header"
    );
    assert!(
        setup_resp.contains("Transport:"),
        "SETUP: missing Transport header"
    );

    let session_id = setup_resp
        .lines()
        .find(|l| l.to_lowercase().starts_with("session:"))
        .and_then(|l| l.split(':').nth(1))
        .map(|v| v.trim().split(';').next().unwrap_or("").trim())
        .unwrap_or("");
    assert!(!session_id.is_empty(), "SETUP: could not parse Session id");

    // PLAY
    let play_req = format!(
        "PLAY {} RTSP/1.0\r\nCSeq: 4\r\nSession: {}\r\n\r\n",
        base_uri, session_id
    );
    let play_resp = rtsp_request(&mut stream, &play_req).expect("PLAY response");
    assert!(
        play_resp.starts_with("RTSP/1.0 200 OK"),
        "PLAY: expected 200 OK, got: {}",
        play_resp.lines().next().unwrap_or("")
    );
    assert!(
        play_resp.contains("RTP-Info:"),
        "PLAY: missing RTP-Info header"
    );

    server.stop();
}
