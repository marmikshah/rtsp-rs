# rtsp-rs

A Rust library for publishing live encoded video over RTSP. Push H.264 frames and any standard client (VLC, ffplay, GStreamer) can play the stream.

Usable from **Rust**, **Python**, **GStreamer**, or as a **standalone CLI**.

> **Status:** Pre-release (`v0.x`). API may change between versions.

## Project structure

```
crates/
├── core/        # rtsp         — core library
├── python/      # rtsp-python  — PyO3 bindings
├── gst/         # gst-rtsp-sink — GStreamer sink plugin
└── cli/         # rtsp-cli     — standalone server binary

examples/
└── python/      # Python demo (numpy + PyAV)
```

## Quick start

### Rust

```rust
use rtsp::Server;

let mut server = Server::new("0.0.0.0:8554");
server.start().unwrap();

// Push H.264 Annex B frames — packetization and delivery are handled internally.
// Timestamp increment = 90000 / fps (e.g. 3000 for 30 fps).
// server.send_frame(&h264_data, 3000).unwrap();
```

### Python

```bash
pip install maturin
maturin develop -m crates/python/Cargo.toml
```

```python
import rtsp

server = rtsp.Server("0.0.0.0:8554")
server.start()

# Push encoded H.264 frames directly — no manual packetization needed.
server.send_frame(h264_bytes, 3000)
```

See `examples/python/test.py` for a full demo that encodes a test pattern with PyAV.

### GStreamer

Build and install the plugin:

```bash
cargo build -p gst-rtsp-sink --release

# Point GStreamer at the built library
export GST_PLUGIN_PATH=$PWD/target/release
```

Stream a test pattern:

```bash
gst-launch-1.0 videotestsrc \
  ! video/x-raw,width=640,height=480,framerate=30/1 \
  ! x264enc tune=zerolatency bitrate=2000 key-int-max=30 \
  ! video/x-h264,stream-format=byte-stream \
  ! rtspserversink port=8554
```

Stream from a webcam (macOS):

```bash
gst-launch-1.0 avfvideosrc \
  ! videoconvert \
  ! x264enc tune=zerolatency bitrate=2000 \
  ! video/x-h264,stream-format=byte-stream \
  ! rtspserversink port=8554
```

Stream from a webcam (Linux):

```bash
gst-launch-1.0 v4l2src device=/dev/video0 \
  ! videoconvert \
  ! x264enc tune=zerolatency bitrate=2000 \
  ! video/x-h264,stream-format=byte-stream \
  ! rtspserversink port=8554
```

Stream from a file:

```bash
gst-launch-1.0 filesrc location=video.mp4 \
  ! qtdemux ! h264parse \
  ! video/x-h264,stream-format=byte-stream \
  ! rtspserversink port=8554
```

Then connect with any RTSP client:

```bash
ffplay rtsp://localhost:8554/stream
vlc rtsp://localhost:8554/stream
```

### CLI

```bash
cargo run -p rtsp-cli -- --bind 0.0.0.0:8554
```

## Building

Requires Rust 1.85+. GStreamer plugin requires `libgstreamer1.0-dev`.

```bash
cargo build -p rtsp                # Core library
cargo build -p gst-rtsp-sink       # GStreamer plugin
cargo build -p rtsp-python         # Python bindings (needs maturin)
cargo build -p rtsp-cli            # CLI binary
cargo test  --workspace            # All tests
cargo clippy --workspace           # Lint
```

## Roadmap

| Version    | Focus       | Key features                                                                            |
| ---------- | ----------- | --------------------------------------------------------------------------------------- |
| **v1.0.0** | H.264 video | Full H.264 support, RFC-compliant SDP/RTP, proper `tracing` logging, SPS/PPS in SDP, CI |
| **v1.1.0** | Signals     | Client connect/disconnect callbacks, session timeout enforcement                        |
| **v1.2.0** | Audio       | Audio support for one codec (AAC or Opus), multi-track SDP                              |
| **v1.3.0** | More codecs | H.265 packetizer (RFC 7798), MJPEG packetizer (RFC 2435)                                |
| **v2.0.0** | Transport   | Interleaved TCP transport, RTCP sender reports, async option                            |

## License

MIT
