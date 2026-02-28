# rtsp-rs

[![Testing](https://github.com/marmikshah/rtsp-rs/actions/workflows/ci-testing.yml/badge.svg?branch=master)](https://github.com/marmikshah/rtsp-rs/actions/workflows/ci-testing.yml)
[![Build](https://github.com/marmikshah/rtsp-rs/actions/workflows/ci-build.yml/badge.svg?branch=master)](https://github.com/marmikshah/rtsp-rs/actions/workflows/ci-build.yml)
[![Release](https://github.com/marmikshah/rtsp-rs/actions/workflows/release.yml/badge.svg)](https://github.com/marmikshah/rtsp-rs/actions/workflows/release.yml)

A Rust library for publishing live encoded video over RTSP. Push frames and play the stream with any standard client (VLC, ffplay, GStreamer).

Usable from **Rust**, **Python**, or **GStreamer**.

---

<div align="center">

**⚠️ Please read before using**

</div>

This is a **personal, hobby project** I built to learn about RTP, RTSP, and streaming. I’m actively evolving it and use it only for my own projects. I have no plans to use or support it in production right now, though I might rely on it there someday if the need arises — so treat it as **use at your own risk**.

Some of the tests, examples, and documentation in this repo were generated or assisted by AI tools. Treat them as reference material rather than guarantees of correctness.

---

> **Status:** Pre-release (`v0.x`). API may change between versions.

## Project structure

```
crates/
├── core/        # rtsp         — core library
├── python/      # rtsp-python  — PyO3 bindings
└── gst/         # gst-rtsp-sink — GStreamer sink plugin

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

## Building

Requires Rust 1.85+. The GStreamer plugin requires `libgstreamer1.0-dev`.

```bash
cargo build -p rtsp                # Core library
cargo build -p gst-rtsp-sink       # GStreamer plugin
cargo build -p rtsp-python         # Python bindings (needs maturin)
cargo test  --workspace            # All tests
cargo clippy --workspace           # Lint
```
