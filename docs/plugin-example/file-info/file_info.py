#!/usr/bin/env python3
"""PDF Seeker demo plugin: print name / size / SHA-256 of the input file.

Uses only the Python standard library. Receives the input path as argv[1]
(the {input} placeholder in plugin.json).
"""
import hashlib
import os
import sys


def main() -> int:
    if len(sys.argv) < 2:
        print("usage: file_info.py <input-pdf>", file=sys.stderr)
        return 2
    path = sys.argv[1]
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    size = os.path.getsize(path)
    print(f"file:  {os.path.basename(path)}")
    print(f"size:  {size:,} bytes ({size / 1024 / 1024:.2f} MiB)")
    print(f"sha256: {h.hexdigest()}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
