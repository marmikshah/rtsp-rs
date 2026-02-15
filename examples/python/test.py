"""
Working RTSP demo that streams generated video frames.
Uses PyAV to encode numpy arrays to H.264.
"""

import rtsp
import time
import numpy as np
import av
from fractions import Fraction


class FrameEncoder:
    """Encodes numpy arrays to H.264 using PyAV."""

    def __init__(self, width: int, height: int, fps: int = 30):
        self.width = width
        self.height = height
        self.fps = fps
        self.frame_count = 0

        # Create encoder
        self.codec = av.CodecContext.create("libx264", "w")
        self.codec.width = width
        self.codec.height = height
        self.codec.pix_fmt = "yuv420p"
        self.codec.time_base = Fraction(1, fps)
        self.codec.framerate = Fraction(fps, 1)

        # Fast encoding settings
        self.codec.options = {
            "preset": "ultrafast",
            "tune": "zerolatency",
            "profile": "baseline",
            "level": "3.1",
        }

        self.codec.open()

        # Store SPS/PPS from first frame
        self.sps_pps = None

    def encode(self, rgb_array: np.ndarray) -> bytes:
        """
        Encode an RGB numpy array to H.264.

        Args:
            rgb_array: numpy array of shape (height, width, 3) with dtype uint8

        Returns:
            Encoded H.264 bytes (Annex B format with start codes)
        """
        # Ensure correct shape
        if rgb_array.shape != (self.height, self.width, 3):
            raise ValueError(
                f"Expected shape ({self.height}, {self.width}, 3), got {rgb_array.shape}"
            )

        # Create video frame
        frame = av.VideoFrame.from_ndarray(rgb_array, format="rgb24")
        frame = frame.reformat(format="yuv420p")
        frame.pts = self.frame_count
        self.frame_count += 1

        # Encode
        packets = self.codec.encode(frame)

        encoded_data = b""
        for packet in packets:
            encoded_data += bytes(packet)

        return encoded_data

    def get_sps_pps(self) -> tuple[bytes, bytes] | None:
        """Get SPS and PPS NAL units from extradata."""
        if self.codec.extradata:
            # Parse AVCC format extradata
            data = bytes(self.codec.extradata)
            if len(data) > 8:
                # This is simplified - real parsing is more complex
                # For baseline profile with PyAV, we might get Annex B format
                return self._parse_extradata(data)
        return None

    def _parse_extradata(self, data: bytes) -> tuple[bytes, bytes] | None:
        """Parse SPS/PPS from codec extradata."""
        # Try to find NAL units in extradata
        sps = None
        pps = None

        parts = data.split(b"\x00\x00\x00\x01")
        for part in parts:
            if len(part) > 0:
                nal_type = part[0] & 0x1F
                if nal_type == 7:  # SPS
                    sps = part
                elif nal_type == 8:  # PPS
                    pps = part

        if sps and pps:
            return (sps, pps)
        return None

    def close(self):
        """Flush encoder and close."""
        # Flush remaining packets
        packets = self.codec.encode(None)
        self.codec.close()


def generate_test_pattern(width: int, height: int, frame_num: int) -> np.ndarray:
    """Generate a colorful moving test pattern."""
    # Create gradient background
    x = np.linspace(0, 1, width)
    y = np.linspace(0, 1, height)
    xx, yy = np.meshgrid(x, y)

    # Animate colors
    t = frame_num / 30.0  # Time in seconds

    r = ((np.sin(xx * 3 + t) + 1) / 2 * 255).astype(np.uint8)
    g = ((np.sin(yy * 3 + t * 1.3) + 1) / 2 * 255).astype(np.uint8)
    b = ((np.sin((xx + yy) * 2 + t * 0.7) + 1) / 2 * 255).astype(np.uint8)

    # Stack into RGB
    frame = np.stack([r, g, b], axis=2)

    # Add a moving box
    box_size = 50
    box_x = int((np.sin(t) + 1) / 2 * (width - box_size))
    box_y = int((np.cos(t * 0.7) + 1) / 2 * (height - box_size))
    frame[box_y : box_y + box_size, box_x : box_x + box_size] = [255, 255, 255]

    return frame


def main():
    # Video settings
    WIDTH = 320
    HEIGHT = 240
    FPS = 30

    # Create server
    server = rtsp.Server("0.0.0.0:8554", public_host="localhost", public_port=8554)
    server.start()

    print("=" * 60)
    print("RTSP H.264 Video Server")
    print("=" * 60)
    print(f"Resolution: {WIDTH}x{HEIGHT} @ {FPS} FPS")
    print()
    print("Connect with VLC:")
    print("  Media -> Open Network Stream")
    print("  URL: rtsp://localhost:8554/test")
    print()
    print("Or use ffplay:")
    print("  ffplay -rtsp_transport udp rtsp://localhost:8554/test")
    print("=" * 60)
    print()

    encoder = FrameEncoder(WIDTH, HEIGHT, FPS)

    frame_num = 0
    frames_sent = 0
    last_status_time = time.time()

    # Timing
    frame_interval = 1.0 / FPS
    next_frame_time = time.monotonic()

    # Pre-generate some frames to prime the encoder
    print("Warming up encoder...")
    for i in range(5):
        frame = generate_test_pattern(WIDTH, HEIGHT, i)
        encoder.encode(frame)
    print("Ready!\n")

    try:
        while True:
            now = time.monotonic()

            if now >= next_frame_time:
                viewers = server.get_viewers()

                if viewers:
                    rgb_frame = generate_test_pattern(WIDTH, HEIGHT, frame_num)
                    encoded = encoder.encode(rgb_frame)

                    if encoded:
                        timestamp_inc = 90000 // FPS  # 90kHz clock
                        server.send_frame(encoded, timestamp_inc)
                        frames_sent += 1

                    frame_num += 1

                next_frame_time += frame_interval

                # Reset if we fall behind
                if next_frame_time < now:
                    next_frame_time = now + frame_interval

            # Status update every second
            if time.time() - last_status_time >= 1.0:
                viewers = server.get_viewers()
                if viewers:
                    print(
                        f"\rFrame: {frame_num:6d} | Sent: {frames_sent:8d} | Viewers: {len(viewers)}",
                        end="",
                        flush=True,
                    )
                else:
                    print(
                        f"\rWaiting for clients... (encoder ready, frame {frame_num})",
                        end="",
                        flush=True,
                    )
                last_status_time = time.time()

            # Small sleep to prevent busy-waiting
            sleep_time = next_frame_time - time.monotonic()
            if sleep_time > 0.001:
                time.sleep(min(sleep_time, 0.01))

    except KeyboardInterrupt:
        print("\n\nShutting down...")
        encoder.close()
        server.stop()


if __name__ == "__main__":
    main()
