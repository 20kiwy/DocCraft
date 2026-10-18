#!/usr/bin/env python3
"""Convert an XWD dump to PNG (handles 24/32bpp ZPixmap, big-endian header)."""
import struct, sys
from PIL import Image

def convert(src, dst):
    data = open(src, "rb").read()
    # XWDFileHeader (25 BE u32) then window name (padded to 4) then colormap
    hdr = struct.unpack(">25I", data[:100])
    header_size = hdr[0]       # covers the 100-byte header + window name
    width, height = hdr[4], hdr[5]
    bpp = hdr[11]              # bits_per_pixel
    bpl = hdr[12]              # bytes_per_line
    ncolors = hdr[19]
    off = header_size + ncolors * 12   # then XWDColor entries
    px = bpp // 8
    raw = data[off:off + bpl * height]
    im = Image.frombytes("RGB", (width, height), raw, "raw", "BGRX" if px == 4 else "BGR")
    im.save(dst)
    print(f"{dst}: {width}x{height}")

if __name__ == "__main__":
    convert(sys.argv[1], sys.argv[2])
