#!/usr/bin/env python3
"""Generate the ISO-2022-CN mapping from GB 2312 and EUC-TW (CNS planes 1–2).

Run on macOS with the system iconv; no iconv is required at runtime.
Entries are little-endian 16-bit Unicode scalars, 0 for an unassigned code.
"""

import ctypes as c
from ctypes.util import find_library
import errno
from pathlib import Path
import struct


iconv = c.CDLL(find_library("iconv") or None, use_errno=True)
iconv.iconv_open.argtypes = [c.c_char_p, c.c_char_p]
iconv.iconv_open.restype = c.c_void_p
iconv.iconv.argtypes = [
    c.c_void_p, c.POINTER(c.c_char_p), c.POINTER(c.c_size_t),
    c.POINTER(c.c_char_p), c.POINTER(c.c_size_t),
]
iconv.iconv.restype = c.c_size_t
iconv.iconv_close.argtypes = [c.c_void_p]
FAILURE = c.c_size_t(-1).value


def cns_points():
    converter = iconv.iconv_open(b"UTF-32LE", b"EUC-TW")
    if converter == c.c_void_p(-1).value:
        raise OSError(c.get_errno(), "iconv_open(EUC-TW)")
    try:
        for prefix in (b"", b"\x8e\xa2"):
            for pair in pairs():
                wire = prefix + pair
                out = c.create_string_buffer(8)
                available = c.c_size_t(len(out))
                left = c.c_size_t(len(wire))
                result = iconv.iconv(converter, c.byref(c.c_char_p(wire)), c.byref(left),
                                     c.byref(c.cast(out, c.c_char_p)), c.byref(available))
                if result == FAILURE:
                    error = c.get_errno()
                    if error != errno.EILSEQ:
                        raise OSError(error, "iconv(EUC-TW)")
                    yield 0
                elif left.value == 0 and available.value == len(out) - 4:
                    yield int.from_bytes(out.raw[:4], "little")
                else:
                    raise ValueError(f"Unexpected EUC-TW mapping: {wire.hex()}")
    finally:
        iconv.iconv_close(converter)


def pairs():
    return (bytes([row, col]) for row in range(0xa1, 0xff) for col in range(0xa1, 0xff))


def gb_points():
    for pair in pairs():
        try:
            yield ord(pair.decode("gb2312"))
        except UnicodeDecodeError:
            yield 0


def main():
    points = [*gb_points(), *cns_points()]
    print("Assigned entries per set:", [sum(p != 0 for p in points[i:i + 94**2])
                                        for i in range(0, len(points), 94**2)])
    table = Path(__file__).resolve().parents[1] / "src/terminal/encoding/iso2022-cn.bin"
    table.write_bytes(struct.pack(f"<{len(points)}H", *points))


if __name__ == "__main__":
    main()
