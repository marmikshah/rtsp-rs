use crate::protocol::{RtspRequest, RtspResponse};

pub fn handle_request(request: &RtspRequest) -> RtspResponse {
    let cseq = request.cseq().unwrap_or("0");

    match request.method.as_str() {
        "OPTIONS" => handle_options(cseq),
        _ => RtspResponse::new(501, "Not Implemented").add_header("CSeq", cseq),
    }
}

fn handle_options(cseq: &str) -> RtspResponse {
    RtspResponse::ok()
        .add_header("CSeq", cseq)
        .add_header("Public", "OPTIONS, DESCRIBE, SETUP, PLAY, PAUSE, TEARDOWN")
}
