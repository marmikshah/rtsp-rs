use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream, UdpSocket, SocketAddr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use crate::handler::RequestHandler;
use crate::protocol::parse_request;
use crate::session::SessionManager;

pub struct RtspServer {
    session_manager: SessionManager,
    running: Arc<AtomicBool>,
    bind_addr: String,
    udp_socket: Option<Arc<UdpSocket>>

}

impl RtspServer {
    pub fn new(bind_addr: &str) -> Self {
        Self {
        session_manager: SessionManager::new(),
        running: Arc::new(AtomicBool::new(false)),
        bind_addr: bind_addr.to_string(),
        udp_socket: None,
        }
    }

    pub fn session_manager(&self,) -> &SessionManager {
        &self.session_manager
    }

    pub fn start(&mut self) -> Result<(), String> {
        if self.running.load(Ordering::SeqCst) {
            return Err("Server already running".to_string());
        }

        let udp_socket = UdpSocket::bind("0.0.0.0:0")
            .map_err(|e| format!("Failed to bind UDP socker: {}", e))?;

        self.udp_socket = Some(Arc::new(udp_socket));

        let listener = TcpListener::bind(&self.bind_addr)
            .map_err(|e| format!("Failed to bind to {}: {}", self.bind_addr, e))?;

        self.running.store(true, Ordering::SeqCst);

        let running = self.running.clone();
        let session_manager = self.session_manager.clone();

        println!("RTSP Server listening on {}", self.bind_addr);


        thread::spawn(move || {
            for stream in listener.incoming() {
                if !running.load(Ordering::SeqCst) {
                    break;
                }
                match stream {
                    Ok(connection) => {
                        let session_manager = session_manager.clone();
                        let running = running.clone();
                        thread::spawn(move || {
                            Self::handle_connection(connection, session_manager, running);
                        });
                    }
                    Err(e) => {
                        if running.load(Ordering::SeqCst) {
                            eprintln!("Connection error: {}", e);
                        }
                    }
                }
            }
        });
        Ok(())
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        println!("Server stopping...");
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    pub fn send_rtp_packet(&self, session_id: &str, payload: &[u8]) -> Result<usize, String> {
        
        let socket = self.udp_socket.as_ref().ok_or("Server not started")?;
        let session = self
            .session_manager
            .get_session(session_id)
            .ok_or("Session not found")?;

        if !session.is_playing() {
            return Err("Session not playing".to_string());
        }

        let transport = session
            .get_transport()
            .ok_or("Transport not configured")?;

        socket
            .send_to(payload, transport.client_addr)
            .map_err(|e| format!("Failed to send packet:  {}",e))


    }

    pub fn broadcast_rtp_packet(&self, payload: &[u8]) -> Result<usize, String> {
        
        let socket = self.udp_socket.as_ref().ok_or("Server not started")?;
        let playing_sessions = self.session_manager.get_playing_sessions();

        if playing_sessions.is_empty() {
            return Ok(0);
        }

        let mut sent = 0;
        for session in playing_sessions {
            if let Some(transport) = session.get_transport() {
                if socket.send_to(payload, transport.client_addr).is_ok() {
                    sent += 1;
                }
            }
        }

        Ok(sent)
        
    }

    pub fn handle_connection(stream: TcpStream, session_manager: SessionManager, running: Arc<AtomicBool>) {
        let peer_addr = match stream.peer_addr() {
            Ok(addr) => addr,
            Err(_) => return,
        };

        println!("Client connected: {}", peer_addr);

        let mut reader = match stream.try_clone() {
            Ok(s) => BufReader::new(s),
            Err(_) => return,
        };

        let mut writer = stream;

        let mut handler = RequestHandler::new(session_manager, peer_addr);

        while running.load(Ordering::SeqCst) {
            let mut request_text = String::new();
            loop {
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Ok(0) => {
                        println!("Client disconnected: {}", peer_addr);
                        return;
                    }
                    Ok(_) => {
                        request_text.push_str(&line);
                        if line == "\r\n"  || line == "\n" {
                            break;
                        }
                    }
                    Err(_) => return,
                }
            }

            if request_text.trim().is_empty() {
                continue;
            }

            println!("[{}] >>> {}", peer_addr, request_text.lines().next().unwrap_or(""));

            match parse_request(&request_text) {
                Ok(request) => {
                    let response = handler.handle(&request);
                    let response_bytes = response.serialize();

                    println!(
                        "[{}] <<< RTSP/1.0 {} {}",
                        peer_addr, response.status_code, response.status_text
                    );

                    if writer.write_all(response_bytes.as_bytes()).is_err() {
                        return;
                    }
                }
                Err(e) => {
                    eprintln!("Parse error: {:?}", e);
                }
            }
        }
    }

    pub fn get_playing_clients(&self) -> Vec<ClientInfo> {
        self.session_manager
            .get_playing_sessions()
            .iter()
            .filter_map(|session| {
                session.get_transport().map(|transport| ClientInfo {
                    session_id: session.id.clone(),
                    uri: session.uri.clone(),
                    client_addr: transport.client_addr.to_string(),
                    client_rtp_port: transport.client_rtp_port,
                })
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct ClientInfo {
    pub session_id: String,
    pub uri: String,
    pub client_addr: String,
    pub client_rtp_port: u16
}
