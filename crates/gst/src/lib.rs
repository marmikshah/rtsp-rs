//! GStreamer sink element for RTSP publishing.
//!
//! Registers `rtspserversink` — a `BaseSink` that accepts encoded H.264
//! (Annex B byte-stream) buffers and publishes them over RTSP to connected
//! clients.
//!
//! ## Loading the plugin
//!
//! After `cargo build -p gst-rtsp-sink`, set `GST_PLUGIN_PATH` to the directory
//! containing the built library so GStreamer can load it:
//!
//! ```text
//! export GST_PLUGIN_PATH="$(pwd)/target/debug"   # or target/release
//! gst-inspect-1.0 rtspserversink
//! ```
//!
//! ## Usage with gst-launch
//!
//! ```text
//! gst-launch-1.0 videotestsrc ! x264enc ! rtspserversink address=0.0.0.0 port=8554
//! gst-launch-1.0 videotestsrc ! x264enc ! rtspserversink port=8554 mount-path=/cam1
//! ```
//!
//! ## Properties
//!
//! | Property      | Type   | Default   | Description                          |
//! |---------------|--------|-----------|--------------------------------------|
//! | `address`     | String | `0.0.0.0` | Address to bind the RTSP server to   |
//! | `port`        | u32    | `8554`    | Port for the RTSP server             |
//! | `mount-path`  | String | `/stream` | RTSP stream path (e.g. /stream, /cam1) |

mod imp;

use gstreamer::glib;
use gstreamer::prelude::*;

glib::wrapper! {
    pub struct RtspServerSink(ObjectSubclass<imp::RtspServerSink>)
        @extends gstreamer_base::BaseSink, gstreamer::Element, gstreamer::Object;
}

fn plugin_init(plugin: &gstreamer::Plugin) -> Result<(), glib::BoolError> {
    gstreamer::Element::register(
        Some(plugin),
        "rtspserversink",
        gstreamer::Rank::NONE,
        RtspServerSink::static_type(),
    )
}

gstreamer::plugin_define!(
    rtspserversink,
    "Create an RTSP Server and publish encoded packets to it",
    plugin_init,
    env!("CARGO_PKG_VERSION"),
    "MIT",
    "gst-rtsp-sink",
    "rtsp",
    "https://github.com/marmikshah/rtsp-rs",
    "2026-02-21"
);
