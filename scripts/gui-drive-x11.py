#!/usr/bin/env python3
"""Run a REWIND GUI program on a Linux X11 display and drive it with real input.

Uses only the Python standard library, libX11 and the XTEST extension (libXtst),
so it needs no extra packages (it works under WSLg).

Usage:
  python3 scripts/gui-drive-x11.py --source main.rw --title "REWIND notes" \
      --steps "clickf:0.5,0.45;keys:hello{ENTER}world;shot:out.png"

Steps (separated by ';'):
  click:X,Y      left click at client pixel coordinates
  clickf:FX,FY   left click at a fraction (0..1) of the client area
  keys:TEXT      type text; {ENTER} {TAB} {BACKSPACE} {LEFT} {RIGHT} {UP} {DOWN} {ESC}
  wait:MS        sleep
  resize:W,H     ask for a client size of W x H
  shot:FILE      save the client area as a PNG
Options: --rewind PATH (default target/release/rewind), --effects, --startup SECONDS.
"""
import argparse
import ctypes
import ctypes.util
import os
import struct
import subprocess
import sys
import time
import zlib

x11 = ctypes.CDLL(ctypes.util.find_library("X11") or "libX11.so.6")
xtst = ctypes.CDLL(ctypes.util.find_library("Xtst") or "libXtst.so.6")

Window = ctypes.c_ulong


class XWindowAttributes(ctypes.Structure):
    _fields_ = [
        ("x", ctypes.c_int), ("y", ctypes.c_int), ("width", ctypes.c_int), ("height", ctypes.c_int),
        ("border_width", ctypes.c_int), ("depth", ctypes.c_int), ("visual", ctypes.c_void_p),
        ("root", Window), ("klass", ctypes.c_int), ("bit_gravity", ctypes.c_int),
        ("win_gravity", ctypes.c_int), ("backing_store", ctypes.c_int),
        ("backing_planes", ctypes.c_ulong), ("backing_pixel", ctypes.c_ulong),
        ("save_under", ctypes.c_int), ("colormap", ctypes.c_ulong), ("map_installed", ctypes.c_int),
        ("map_state", ctypes.c_int), ("all_event_masks", ctypes.c_long),
        ("your_event_mask", ctypes.c_long), ("do_not_propagate_mask", ctypes.c_long),
        ("override_redirect", ctypes.c_int), ("screen", ctypes.c_void_p),
    ]


class XImage(ctypes.Structure):
    _fields_ = [
        ("width", ctypes.c_int), ("height", ctypes.c_int), ("xoffset", ctypes.c_int),
        ("format", ctypes.c_int), ("data", ctypes.POINTER(ctypes.c_ubyte)),
        ("byte_order", ctypes.c_int), ("bitmap_unit", ctypes.c_int),
        ("bitmap_bit_order", ctypes.c_int), ("bitmap_pad", ctypes.c_int), ("depth", ctypes.c_int),
        ("bytes_per_line", ctypes.c_int), ("bits_per_pixel", ctypes.c_int),
        ("red_mask", ctypes.c_ulong), ("green_mask", ctypes.c_ulong), ("blue_mask", ctypes.c_ulong),
    ]


x11.XOpenDisplay.restype = ctypes.c_void_p
x11.XOpenDisplay.argtypes = [ctypes.c_char_p]
x11.XDefaultRootWindow.restype = Window
x11.XDefaultRootWindow.argtypes = [ctypes.c_void_p]
x11.XQueryTree.argtypes = [ctypes.c_void_p, Window, ctypes.POINTER(Window), ctypes.POINTER(Window),
                           ctypes.POINTER(ctypes.POINTER(Window)), ctypes.POINTER(ctypes.c_uint)]
x11.XFetchName.argtypes = [ctypes.c_void_p, Window, ctypes.POINTER(ctypes.c_char_p)]
x11.XGetWindowAttributes.argtypes = [ctypes.c_void_p, Window, ctypes.POINTER(XWindowAttributes)]
x11.XTranslateCoordinates.argtypes = [ctypes.c_void_p, Window, Window, ctypes.c_int, ctypes.c_int,
                                      ctypes.POINTER(ctypes.c_int), ctypes.POINTER(ctypes.c_int),
                                      ctypes.POINTER(Window)]
x11.XGetImage.restype = ctypes.POINTER(XImage)
x11.XGetImage.argtypes = [ctypes.c_void_p, Window, ctypes.c_int, ctypes.c_int, ctypes.c_uint,
                          ctypes.c_uint, ctypes.c_ulong, ctypes.c_int]
x11.XKeysymToKeycode.restype = ctypes.c_ubyte
x11.XKeysymToKeycode.argtypes = [ctypes.c_void_p, ctypes.c_ulong]
x11.XKeycodeToKeysym.restype = ctypes.c_ulong
x11.XKeycodeToKeysym.argtypes = [ctypes.c_void_p, ctypes.c_ubyte, ctypes.c_int]
x11.XResizeWindow.argtypes = [ctypes.c_void_p, Window, ctypes.c_uint, ctypes.c_uint]
x11.XFlush.argtypes = [ctypes.c_void_p]
x11.XSync.argtypes = [ctypes.c_void_p, ctypes.c_int]
x11.XFree.argtypes = [ctypes.c_void_p]
xtst.XTestFakeMotionEvent.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_int, ctypes.c_int,
                                      ctypes.c_ulong]
xtst.XTestFakeButtonEvent.argtypes = [ctypes.c_void_p, ctypes.c_uint, ctypes.c_int, ctypes.c_ulong]
xtst.XTestFakeKeyEvent.argtypes = [ctypes.c_void_p, ctypes.c_uint, ctypes.c_int, ctypes.c_ulong]

SPECIAL = {
    "ENTER": 0xFF0D, "TAB": 0xFF09, "BACKSPACE": 0xFF08, "ESC": 0xFF1B,
    "LEFT": 0xFF51, "UP": 0xFF52, "RIGHT": 0xFF53, "DOWN": 0xFF54,
}
SHIFT = 0xFFE1


def find_window(display, root, title):
    stack = [root]
    while stack:
        window = stack.pop()
        name = ctypes.c_char_p()
        if x11.XFetchName(display, window, ctypes.byref(name)) and name.value is not None:
            matches = name.value.decode("latin-1") == title
            x11.XFree(name)
            if matches:
                attributes = XWindowAttributes()
                x11.XGetWindowAttributes(display, window, ctypes.byref(attributes))
                if attributes.map_state == 2:
                    return window
        root_return, parent, children, count = Window(), Window(), ctypes.POINTER(Window)(), ctypes.c_uint()
        if x11.XQueryTree(display, window, ctypes.byref(root_return), ctypes.byref(parent),
                          ctypes.byref(children), ctypes.byref(count)):
            for index in range(count.value):
                stack.append(children[index])
            if children:
                x11.XFree(children)
    return None


def client_geometry(display, root, window):
    attributes = XWindowAttributes()
    x11.XGetWindowAttributes(display, window, ctypes.byref(attributes))
    x, y, child = ctypes.c_int(), ctypes.c_int(), Window()
    x11.XTranslateCoordinates(display, window, root, 0, 0, ctypes.byref(x), ctypes.byref(y), ctypes.byref(child))
    return x.value, y.value, attributes.width, attributes.height


def click(display, x, y):
    xtst.XTestFakeMotionEvent(display, -1, x, y, 0)
    x11.XSync(display, 0)
    time.sleep(0.2)
    xtst.XTestFakeButtonEvent(display, 1, 1, 0)
    x11.XSync(display, 0)
    time.sleep(0.08)
    xtst.XTestFakeButtonEvent(display, 1, 0, 0)
    x11.XSync(display, 0)
    time.sleep(0.5)


def press(display, keysym, shift=False):
    code = x11.XKeysymToKeycode(display, keysym)
    if not code:
        raise SystemExit(f"no keycode for keysym {keysym:#x}")
    if shift:
        xtst.XTestFakeKeyEvent(display, x11.XKeysymToKeycode(display, SHIFT), 1, 0)
    xtst.XTestFakeKeyEvent(display, code, 1, 0)
    xtst.XTestFakeKeyEvent(display, code, 0, 0)
    if shift:
        xtst.XTestFakeKeyEvent(display, x11.XKeysymToKeycode(display, SHIFT), 0, 0)
    x11.XSync(display, 0)
    time.sleep(0.03)


def type_text(display, text):
    index = 0
    while index < len(text):
        if text[index] == "{" and "}" in text[index:]:
            close = text.index("}", index)
            press(display, SPECIAL[text[index + 1:close]])
            index = close + 1
            continue
        keysym = ord(text[index])
        code = x11.XKeysymToKeycode(display, keysym)
        shift = bool(code) and x11.XKeycodeToKeysym(display, code, 0) != keysym
        press(display, keysym, shift)
        index += 1
    time.sleep(0.4)


def save_png(display, window, path):
    image = x11.XGetImage(display, window, 0, 0, *client_size(display, window), 0xFFFFFFFF, 2)
    if not image:
        raise SystemExit("XGetImage failed")
    info = image.contents
    rows = []
    stride = info.bytes_per_line
    raw = ctypes.string_at(info.data, stride * info.height)
    for row in range(info.height):
        line = raw[row * stride:row * stride + info.width * 4]
        rgb = bytearray()
        for pixel in range(info.width):
            b, g, r = line[pixel * 4], line[pixel * 4 + 1], line[pixel * 4 + 2]
            rgb += bytes((r, g, b))
        rows.append(b"\x00" + bytes(rgb))
    data = zlib.compress(b"".join(rows))

    def chunk(kind, body):
        return struct.pack(">I", len(body)) + kind + body + struct.pack(">I", zlib.crc32(kind + body))

    png = (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", info.width, info.height, 8, 2, 0, 0, 0))
           + chunk(b"IDAT", data) + chunk(b"IEND", b""))
    with open(path, "wb") as handle:
        handle.write(png)


def client_size(display, window):
    attributes = XWindowAttributes()
    x11.XGetWindowAttributes(display, window, ctypes.byref(attributes))
    return attributes.width, attributes.height


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--source", required=True)
    parser.add_argument("--title", required=True)
    parser.add_argument("--steps", default="")
    parser.add_argument("--effects", default="gui")
    parser.add_argument("--rewind", default=os.path.join(os.path.dirname(__file__), "..", "target", "release", "rewind"))
    parser.add_argument("--startup", type=float, default=2.5)
    args = parser.parse_args()
    display = x11.XOpenDisplay(None)
    if not display:
        raise SystemExit("cannot open the X display (is DISPLAY set?)")
    root = x11.XDefaultRootWindow(display)
    source = os.path.abspath(args.source)
    process = subprocess.Popen([args.rewind, "run", source, "--allow-effects", args.effects],
                               cwd=os.path.dirname(source))
    try:
        time.sleep(args.startup)
        window = find_window(display, root, args.title)
        if window is None:
            raise SystemExit(f"window {args.title!r} was not found")
        for step in filter(None, (s.strip() for s in args.steps.split(";"))):
            name, _, argument = step.partition(":")
            x, y, width, height = client_geometry(display, root, window)
            if name == "click":
                px, py = (int(v) for v in argument.split(","))
                click(display, x + px, y + py)
            elif name == "clickf":
                fx, fy = (float(v) for v in argument.split(","))
                click(display, x + int(width * fx), y + int(height * fy))
            elif name == "keys":
                type_text(display, argument)
            elif name == "wait":
                time.sleep(int(argument) / 1000)
            elif name == "resize":
                w, h = (int(v) for v in argument.split(","))
                x11.XResizeWindow(display, window, w, h)
                x11.XSync(display, 0)
                time.sleep(0.8)
            elif name == "shot":
                save_png(display, window, argument)
            else:
                raise SystemExit(f"unknown step {name}")
    finally:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()


if __name__ == "__main__":
    sys.exit(main())
