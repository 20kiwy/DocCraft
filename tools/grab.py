#!/usr/bin/env python3
"""Grab the X11 root window (or a region) and save as PNG.

Usage: grab.py OUT.png [W H [X Y]]
Uses libX11 via ctypes only; pixels come back as ZPixmap BGRA.
"""
import ctypes, sys
from PIL import Image

def grab(out, w=None, h=None, x=0, y=0):
    x11 = ctypes.CDLL("libX11.so.6")
    x11.XOpenDisplay.restype = ctypes.c_void_p
    x11.XOpenDisplay.argtypes = [ctypes.c_char_p]
    x11.XDefaultScreen.argtypes = [ctypes.c_void_p]
    x11.XRootWindow.argtypes = [ctypes.c_void_p, ctypes.c_int]
    x11.XGetImage.argtypes = [ctypes.c_void_p, ctypes.c_long, ctypes.c_int, ctypes.c_int,
                              ctypes.c_uint, ctypes.c_uint, ctypes.c_ulong, ctypes.c_int]
    x11.XGetImage.restype = ctypes.c_void_p
    x11.XDestroyImage.argtypes = [ctypes.c_void_p]
    W, H = w, h
    d = x11.XOpenDisplay(None)
    if not d:
        sys.exit("cannot open display")
    if W is None:
        W = x11.XDisplayWidth(d, x11.XDefaultScreen(d))
        H = x11.XDisplayHeight(d, x11.XDefaultScreen(d))
    ZPIXMAP = 2
    ALLPLANES = 0xFFFFFFFF
    img = x11.XGetImage(d, x11.XRootWindow(d, x11.XDefaultScreen(d)), x, y, W, H, ALLPLANES, ZPIXMAP)
    if not img:
        sys.exit("XGetImage failed")
    # struct _XImage offsets (amd64 linux): width=16, height=20, bpp=48, bpl=56, data=96
    width = ctypes.c_int.from_address(img + 16).value
    height = ctypes.c_int.from_address(img + 20).value
    bpp = ctypes.c_int.from_address(img + 48).value
    bpl = ctypes.c_int.from_address(img + 56).value
    data_ptr = ctypes.c_void_p.from_address(img + 96).value
    raw = ctypes.string_at(data_ptr, bpl * height)
    x11.XDestroyImage(img)
    px = bpp // 8
    rows = [raw[r * bpl: r * bpl + width * px] for r in range(height)]
    buf = b"".join(rows)
    if px == 4:
        im = Image.frombytes("RGB", (width, height), buf, "raw", "BGRX")
    elif px == 3:
        im = Image.frombytes("RGB", (width, height), buf, "raw", "BGR")
    else:
        sys.exit(f"unsupported bpp {bpp}")
    im.save(out)
    print(f"{out}: {width}x{height}")

if __name__ == "__main__":
    out = sys.argv[1]
    w = int(sys.argv[2]) if len(sys.argv) > 2 else None
    h = int(sys.argv[3]) if len(sys.argv) > 3 else None
    x = int(sys.argv[4]) if len(sys.argv) > 4 else 0
    y = int(sys.argv[5]) if len(sys.argv) > 5 else 0
    grab(out, w, h, x, y)
