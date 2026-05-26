#!/usr/bin/env python3
"""One-shot script: generate tap/catalog/amiga/*.toml from Amiga Forever .rp9 archives.

Usage:
    python3 scripts/generate-amiga-catalog.py <games_dir> <output_dir>

Example:
    python3 scripts/generate-amiga-catalog.py \
        "/home/derek/Documents/FS-UAE/AmigaForever/Amiga Files/Titles/Games" \
        tap/catalog/amiga
"""

import os
import re
import sys
import zipfile
import xml.etree.ElementTree as ET

NS = "http://www.retroplatform.com"

SYSTEM_MAP = {
    "a-500":     "a500",
    "a-500plus": "a500",
    "a-600":     "a600",
    "a-1200":    "a1200",
    "a-2000":    "a500",
    "a-4000":    "a4000",
}

# Games already in the catalog under a different ID — skip to avoid duplicates.
SKIP_TITLES = {"Fatman - The Caped Consumer"}


def slugify(title: str) -> str:
    s = title.lower()
    for prefix in ("the ", "a ", "an "):
        if s.startswith(prefix):
            s = s[len(prefix):]
            break
    s = re.sub(r"[^a-z0-9]+", "-", s)
    return s.strip("-")


def toml_str(value: str) -> str:
    escaped = value.replace("\\", "\\\\").replace('"', '\\"')
    return f'"{escaped}"'


def parse_manifest(xml_bytes: bytes, rp9_filename: str) -> dict | None:
    try:
        root = ET.fromstring(xml_bytes.decode("utf-8-sig"))
    except ET.ParseError as e:
        print(f"  WARNING: XML parse error in {rp9_filename}: {e}", file=sys.stderr)
        return None

    def find(path):
        return root.find(f".//{{{NS}}}{path}")

    def findall(path):
        return root.findall(f".//{{{NS}}}{path}")

    title_el = find("title")
    if title_el is None or not title_el.text:
        print(f"  WARNING: no title in {rp9_filename}, skipping", file=sys.stderr)
        return None
    title = title_el.text.strip()

    if title in SKIP_TITLES:
        print(f"  Skipping {title!r} (already in catalog)")
        return None

    system_el = find("system")
    if system_el is None or not system_el.text:
        print(f"  WARNING: no system in {rp9_filename}, skipping", file=sys.stderr)
        return None
    model = SYSTEM_MAP.get(system_el.text.strip(), "a500")

    year_el = find("year")
    year = int(year_el.text.strip()) if year_el is not None and year_el.text else None

    entity_el = find("entity[@type='publisher']")
    publisher = entity_el.text.strip() if entity_el is not None and entity_el.text else None

    genre_el = find("genre")
    genre_tags: list[str] = []
    if genre_el is not None and genre_el.text:
        genre_tags = [g for g in genre_el.text.strip().split("-") if g]

    # Collect floppies sorted by priority attribute
    floppy_els = sorted(
        findall("floppy"),
        key=lambda e: int(e.get("priority", "99")),
    )
    floppies = [e.text.strip() for e in floppy_els if e.text]

    hd_els = sorted(
        findall("harddrive"),
        key=lambda e: int(e.get("priority", "99")),
    )
    hard_drives = [e.text.strip() for e in hd_els if e.text]

    return {
        "id": slugify(title),
        "title": title,
        "year": year,
        "publisher": publisher,
        "genre": genre_tags,
        "model": model,
        "floppies": floppies,
        "hard_drives": hard_drives,
        "rp9_filename": rp9_filename,
    }


def render_toml(data: dict) -> str:
    lines = ["schema_version = 1", ""]

    lines += [
        "[game]",
        f"id       = {toml_str(data['id'])}",
        f"title    = {toml_str(data['title'])}",
        'platform = "amiga"',
        "",
    ]

    lines.append("[meta]")
    if data["year"]:
        lines.append(f"year      = {data['year']}")
    if data["publisher"]:
        lines.append(f"developer = {toml_str(data['publisher'])}")
        lines.append(f"publisher = {toml_str(data['publisher'])}")
    if data["genre"]:
        arr = ", ".join(toml_str(g) for g in data["genre"])
        lines.append(f"genre     = [{arr}]")
    lines.append("")

    lines += [
        "[acquisition]",
        'amiga_forever = "https://www.amigaforever.com/plus/"',
        "",
    ]

    lines += [
        "[install]",
        f"expects_files = [{toml_str(data['rp9_filename'])}]",
        "",
    ]

    lines += [
        "[runtime]",
        'emulator = "fs-uae"',
        "",
        "[runtime.fs_uae]",
        f"model    = {toml_str(data['model'])}",
    ]

    if data["floppies"]:
        arr = ", ".join(toml_str(f) for f in data["floppies"])
        lines.append(f"floppies = [{arr}]")
    elif data["hard_drives"]:
        arr = ", ".join(toml_str(h) for h in data["hard_drives"])
        lines.append(f"hard_drives = [{arr}]")

    lines.append("")
    return "\n".join(lines)


def main():
    if len(sys.argv) != 3:
        print(__doc__)
        sys.exit(1)

    games_dir = sys.argv[1]
    out_dir = sys.argv[2]

    rp9_files = sorted(f for f in os.listdir(games_dir) if f.endswith(".rp9"))
    print(f"Found {len(rp9_files)} .rp9 files")

    written = 0
    skipped = 0

    for rp9_name in rp9_files:
        rp9_path = os.path.join(games_dir, rp9_name)
        try:
            with zipfile.ZipFile(rp9_path) as z:
                with z.open("rp9-manifest.xml") as f:
                    xml_bytes = f.read()
        except Exception as e:
            print(f"  WARNING: could not open {rp9_name}: {e}", file=sys.stderr)
            continue

        data = parse_manifest(xml_bytes, rp9_name)
        if data is None:
            skipped += 1
            continue

        out_path = os.path.join(out_dir, f"{data['id']}.toml")
        if os.path.exists(out_path):
            print(f"  Skipping {data['id']!r} (output file already exists)")
            skipped += 1
            continue

        toml_content = render_toml(data)
        with open(out_path, "w", encoding="utf-8") as f:
            f.write(toml_content)
        print(f"  Wrote {data['id']}.toml  ({data['title']!r})")
        written += 1

    print(f"\nDone: {written} written, {skipped} skipped")


if __name__ == "__main__":
    main()
