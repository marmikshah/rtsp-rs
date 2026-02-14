use crate::protocol::{RtspRequest, RtspResponse, parse_transport_header};
use crate::session::{PlaybackState, SessionManager, Transport};
use std::net::SocketAddr;

pub struct RequestHandler {
    session_manager: SessionManager,
    client_addr: SocketAddr,
}

impl RequestHandler {
    pub fn new(session_manager: SessionManager, client_addr: SocketAddr) -> Self {
        RequestHandler {
            session_manager,
            client_addr,
        }
    }

    pub fn handle(&mut self, request: &RtspRequest) -> RtspResponse {
        let cseq = request.cseq().unwrap_or("0");

        match request.method.as_str() {
            "OPTIONS" => self.handle_options(cseq),
            "DESCRIBE" => self.handle_describe(cseq, &request.uri),
            "SETUP" => self.handle_setup(cseq, &request),
            "PLAY" => self.handle_play(cseq, &request),
            "PAUSE" => self.handle_pause(cseq, &request),
            "TEARDOWN" => self.handle_teardown(cseq, &request),
            _ => RtspResponse::new(501, "Not Implemented").add_header("CSeq", cseq),
        }
    }

    fn handle_options(&self, cseq: &str) -> RtspResponse {
        RtspResponse::ok()
            .add_header("CSeq", cseq)
            .add_header("Public", "OPTIONS, DESCRIBE, SETUP, PLAY, PAUSE, TEARDOWN")
    }

    fn handle_describe(&self, cseq: &str, uri: &str) -> RtspResponse {
        // A minimal SDP for Single H264 stream
        // Session Desciption Protocol (SDP)
        let sdp = format!(
            "v=0\r\n
            o=- 0.0 IN IP4 127.0.0.1\r\n\
            s=RTSP Server\r\n
            c=IN IP4 0.0.0.0\r\n\
            t=0 0\r\n\
            m=Video 0 RTP/AVP 96\r\n\
            a=rtpmap:96 H264/90000\r\n\
            a=control:track1\r\n"
        );

        RtspResponse::ok()
            .add_header("CSeq", cseq)
            .add_header("Content-Type", "application/sdp")
            .add_header("Content-Base", uri)
            .with_body(sdp)
    }

    fn handle_setup(&mut self, cseq: &str, request: &RtspRequest) -> RtspResponse {
        let transport_header = match request.get_header("Transport") {
            Some(t) => t,
            None => {
                return RtspResponse::bad_request().add_header("CSeq", cseq);
            }
        };

        let client_transport = match parse_transport_header(transport_header) {
            Some(t) => t,
            None => {
                return RtspResponse::bad_request().add_header("CSeq", cseq);
            }
        };

        let (server_rtp_port, server_rtcp_port) = self.session_manager.allocate_server_ports();

        let session = self.session_manager.create_session(&request.uri);

        let session_id = session.id.clone();
        let client_rtp_addr =
            SocketAddr::new(self.client_addr.ip(), client_transport.client_rtp_port);

        session.set_transport(Transport {
            client_rtp_port: client_transport.client_rtp_port,
            client_rtcp_port: client_transport.client_rtcp_port,
            server_rtp_port,
            server_rtcp_port,
            client_addr: client_rtp_addr,
        });

        let transport_response = format!(
            "RTP/AVP;unicast;client_port={}-{};server_port={}-{}",
            client_transport.client_rtp_port,
            client_transport.client_rtcp_port,
            server_rtp_port,
            server_rtcp_port
        );

        RtspResponse::ok()
            .add_header("CSeq", cseq)
            .add_header("Transport", &transport_response)
            .add_header("Session", &session_id)
    }

    fn handle_play(&mut self, cseq: &str, request: &RtspRequest) -> RtspResponse {
        let session_id = match request.get_header("Session") {
            Some(s) => s,
            None => {
                return RtspResponse::new(454, "Session Not Found").add_header("CSeq", cseq);
            }
        };

        match self.session_manager.get_session(session_id) {
            Some(session) => {
                session.set_state(PlaybackState::Playing);
                println!("Session {} started playing", session_id);
                RtspResponse::ok()
                    .add_header("CSeq", cseq)
                    .add_header("Session", session_id)
                    .add_header("Range", "npt=0.000-")
            }
            None => RtspResponse::new(454, "Session Not Found").add_header("CSeq", cseq),
        }
    }

    fn handle_pause(&mut self, cseq: &str, request: &RtspRequest) -> RtspResponse {
        let session_id = match request.get_header("Session") {
            Some(s) => s,
            None => {
                return RtspResponse::new(454, "Session Not Found").add_header("CSeq", cseq);
            }
        };

        match self.session_manager.get_session(session_id) {
            Some(session) => {
                session.set_state(PlaybackState::Paused);
                println!("Session {} paused", session_id);

                RtspResponse::ok()
                    .add_header("CSeq", cseq)
                    .add_header("Session", session_id)
            }
            None => RtspResponse::new(454, "Session Not Found").add_header("CSeq", cseq),
        }
    }

    fn handle_teardown(&mut self, cseq: &str, request: &RtspRequest) -> RtspResponse {
        let session_id = match request.get_header("Session") {
            Some(s) => s,
            None => {
                return RtspResponse::new(454, "Session Not Found").add_header("CSeq", cseq);
            }
        };

        match self.session_manager.remove_session(session_id) {
            Some(_) => {
                println!("Session {} terminated", session_id);

                RtspResponse::ok().add_header("CSeq", cseq)
            }
            None => RtspResponse::new(454, "Session Not Found").add_header("CSeq", cseq),
        }
    }
}
