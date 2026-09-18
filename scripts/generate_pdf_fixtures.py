#!/usr/bin/env python3
"""Generate the deterministic PDF fixtures used by Rust regression tests.

This script uses only the Python standard library and writes minimal PDF 1.4
files. Do not add customer documents or OCR models to this fixture directory.
"""

from pathlib import Path
import struct
import zlib

FIXTURE_DIR = Path(__file__).resolve().parents[1] / "src-tauri" / "tests" / "fixtures"


def escape_pdf_text(value: str) -> str:
    return value.replace("\\", "\\\\").replace("(", "\\(").replace(")", "\\)")


def build_pdf(page_texts: list[str]) -> bytes:
    page_ids = [4 + index * 2 for index in range(len(page_texts))]
    content_ids = [page_id + 1 for page_id in page_ids]
    objects: list[tuple[int, str]] = [
        (1, "<< /Type /Catalog /Pages 2 0 R >>"),
        (
            2,
            "<< /Type /Pages /Count "
            f"{len(page_ids)} /Kids [{' '.join(f'{page_id} 0 R' for page_id in page_ids)}] >>",
        ),
        (3, "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>"),
    ]

    for index, text in enumerate(page_texts):
        page_id = page_ids[index]
        content_id = content_ids[index]
        stream = f"BT /F1 18 Tf 72 720 Td ({escape_pdf_text(text)}) Tj ET"
        objects.append(
            (
                page_id,
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] "
                "/Resources << /Font << /F1 3 0 R >> >> "
                f"/Contents {content_id} 0 R >>",
            )
        )
        objects.append((content_id, f"<< /Length {len(stream.encode('ascii'))} >>\nstream\n{stream}\nendstream"))

    objects.sort(key=lambda item: item[0])
    payload = bytearray(b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n")
    offsets = [0] * (objects[-1][0] + 1)
    for object_id, body in objects:
        offsets[object_id] = len(payload)
        payload.extend(f"{object_id} 0 obj\n{body}\nendobj\n".encode("ascii"))

    xref_offset = len(payload)
    payload.extend(f"xref\n0 {len(offsets)}\n".encode("ascii"))
    payload.extend(b"0000000000 65535 f \n")
    for offset in offsets[1:]:
        payload.extend(f"{offset:010d} 00000 n \n".encode("ascii"))
    payload.extend(
        f"trailer\n<< /Size {len(offsets)} /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n".encode("ascii")
    )
    return bytes(payload)


def build_png_fixture() -> bytes:
    width, height = 2, 2
    # RGBA pixels: red, green / blue, white.
    raw = bytes([
        0, 255, 0, 0, 255, 0, 255, 0, 255, 0,
        0, 0, 255, 255, 255, 255, 255, 255, 255, 255,
    ])

    def chunk(kind: bytes, data: bytes) -> bytes:
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data) & 0xffffffff)

    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(raw))
        + chunk(b"IEND", b"")
    )


def build_corrupted_pdf() -> bytes:
    return b"%PDF-1.4\n1 0 obj\n<< /Type /Catalog >>\nendobj\n%%TRUNCATED"


def build_rotated_contents_array_pdf() -> bytes:
    first_stream = "BT /F1 18 Tf 72 720 Td (PDF Seeker Fixture: content-array part one) Tj ET"
    second_stream = "BT /F1 18 Tf 72 680 Td (PDF Seeker Fixture: content-array part two) Tj ET"
    objects = [
        (1, "<< /Type /Catalog /Pages 2 0 R >>"),
        (2, "<< /Type /Pages /Count 1 /Kids [4 0 R] >>"),
        (3, "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>"),
        (
            4,
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Rotate 90 "
            "/Resources << /Font << /F1 3 0 R >> >> /Contents [5 0 R 6 0 R] >>",
        ),
        (5, f"<< /Length {len(first_stream.encode('ascii'))} >>\nstream\n{first_stream}\nendstream"),
        (6, f"<< /Length {len(second_stream.encode('ascii'))} >>\nstream\n{second_stream}\nendstream"),
    ]

    payload = bytearray(b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n")
    offsets = [0] * 7
    for object_id, body in objects:
        offsets[object_id] = len(payload)
        payload.extend(f"{object_id} 0 obj\n{body}\nendobj\n".encode("ascii"))

    xref_offset = len(payload)
    payload.extend(b"xref\n0 7\n0000000000 65535 f \n")
    for offset in offsets[1:]:
        payload.extend(f"{offset:010d} 00000 n \n".encode("ascii"))
    payload.extend(f"trailer\n<< /Size 7 /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n".encode("ascii"))
    return bytes(payload)


def build_chinese_pdf() -> bytes:
    return build_pdf([
        "PDF Seeker Fixture: CHINESE-FIXTURE-MARK Page 1",
        "PDF Seeker Fixture: CHINESE-FIXTURE-MARK Page 2",
    ])


def build_encrypted_pdf() -> bytes:
    objects = [
        (1, "<< /Type /Catalog /Pages 2 0 R >>"),
        (2, "<< /Type /Pages /Count 1 /Kids [4 0 R] >>"),
        (3, "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>"),
        (
            4,
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] "
            "/Resources << /Font << /F1 3 0 R >> >> "
            "/Contents 5 0 R >>",
        ),
        (5, "<< /Length 40 >>\nstream\nBT /F1 18 Tf 72 720 Td (Encrypted) Tj ET\nendstream"),
        (6, "<< /Filter /Standard /V 1 /R 2 /O (12345678901234567890123456789012) /U (12345678901234567890123456789012) /P -4 >>"),
    ]
    objects.sort(key=lambda item: item[0])
    payload = bytearray(b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n")
    offsets = [0] * (objects[-1][0] + 1)
    for object_id, body in objects:
        offsets[object_id] = len(payload)
        payload.extend(f"{object_id} 0 obj\n{body}\nendobj\n".encode("ascii"))

    xref_offset = len(payload)
    payload.extend(f"xref\n0 {len(offsets)}\n".encode("ascii"))
    payload.extend(b"0000000000 65535 f \n")
    for offset in offsets[1:]:
        payload.extend(f"{offset:010d} 00000 n \n".encode("ascii"))
    payload.extend(
        f"trailer\n<< /Size {len(offsets)} /Root 1 0 R /Encrypt 6 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n".encode("ascii")
    )
    return bytes(payload)


def main() -> None:
    FIXTURE_DIR.mkdir(parents=True, exist_ok=True)
    fixtures = {
        "single-page-text.pdf": ["PDF Seeker Fixture: single-page text"],
        "three-page-target.pdf": [
            "PDF Seeker Fixture: target page 1",
            "PDF Seeker Fixture: target page 2",
            "PDF Seeker Fixture: target page 3",
        ],
        "two-page-source.pdf": [
            "PDF Seeker Fixture: source page 1",
            "PDF Seeker Fixture: source page 2",
        ],
    }
    for name, pages in fixtures.items():
        (FIXTURE_DIR / name).write_bytes(build_pdf(pages))
        print(f"wrote {name}: {len(pages)} page(s)")

    chinese_name = "chinese-text.pdf"
    (FIXTURE_DIR / chinese_name).write_bytes(build_chinese_pdf())
    print(f"wrote {chinese_name}: 2 pages with Chinese fixture marker")

    encrypted_name = "encrypted-test.pdf"
    (FIXTURE_DIR / encrypted_name).write_bytes(build_encrypted_pdf())
    print(f"wrote {encrypted_name}: encrypted test PDF")

    special_name = "rotated-contents-array.pdf"
    (FIXTURE_DIR / special_name).write_bytes(build_rotated_contents_array_pdf())
    print(f"wrote {special_name}: 1 rotated page with two content streams")

    image_name = "two-by-two-rgba.png"
    (FIXTURE_DIR / image_name).write_bytes(build_png_fixture())
    print(f"wrote {image_name}: 2x2 RGBA PNG")

    corrupt_name = "truncated-input.pdf"
    (FIXTURE_DIR / corrupt_name).write_bytes(build_corrupted_pdf())
    print(f"wrote {corrupt_name}: deliberately truncated PDF")


if __name__ == "__main__":
    main()
