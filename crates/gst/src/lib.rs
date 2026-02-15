//! GStreamer sink element for RTSP publishing.
//!
//! Registers `rtspserversink` — a `BaseSink` that accepts encoded H.264
//! (Annex B byte-stream) buffers and publishes them over RTSP to connected
//! clients.
//!
//! ## Usage with gst-launch
//!
//! ```text
//! gst-launch-1.0 videotestsrc ! x264enc ! rtspserversink address=0.0.0.0 port=8554
//! ```
//!
//! ## Properties
//!
//! | Property  | Type   | Default     | Description                        |
//! |-----------|--------|-------------|------------------------------------|
//! | `address` | String | `0.0.0.0`   | Address to bind the RTSP server to |
//! | `port`    | u32    | `8554`      | Port for the RTSP server           |

mod imp;

use gst::glib;
use gst::prelude::*;

glib::wrapper! {
    pub struct RtspServerSink(ObjectSubclass<imp::RtspServerSink>)
        @extends gst_base::BaseSink, gst::Element, gst::Object;
}

fn plugin_init(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "rtspserversink",
        gst::Rank::NONE,
        RtspServerSink::static_type(),
    )
}

gst::plugin_define!(
    rtspserversink,
    "Create an RTSP Server and publish encoded packets to it",
    plugin_init,
    env!("CARGO_PKG_VERSION"),
    "MIT",
    "gst-rtsp-sink",
    "rtsp",
    "https://github.com/marmikshah/rtsp",
    "2026-02-21"
);
