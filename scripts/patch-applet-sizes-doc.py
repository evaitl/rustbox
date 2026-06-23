#!/usr/bin/env python3
"""Patch APPLETS.md with marginal binary size lines."""
from __future__ import annotations

import json
import re
from pathlib import Path

DATA = Path(__file__).resolve().parent / "applet-sizes.json"
payload = json.loads(DATA.read_text())
FULL: int = payload["full"]
SIZES: dict[str, int] = payload["marginal_bytes"]

def fmt_kb(n: int) -> str:
    if n == 0:
        return "~0 KiB"
    kb = n / 1024
    if kb >= 100:
        return f"~{round(kb)} KiB"
    if abs(kb - round(kb)) < 0.05:
        return f"~{round(kb)} KiB"
    return f"~{kb:.1f} KiB"


def size_line(name: str) -> str:
    kb = fmt_kb(SIZES[name])
    line = f"**Approximate binary size** — {kb} marginal."
    extra = {
        "rash": " `sh` is an alias for the same module.",
        "test": " `[` is an alias for the same module; required when `rash`/`sh` is enabled.",
        "cat": " Below page-alignment resolution; folded into shared runtime.",
        "false": " Below page-alignment resolution; folded into shared runtime.",
        "halt": " Below page-alignment resolution; folded into shared runtime.",
        "pwd": " Below page-alignment resolution; folded into shared runtime.",
        "reboot": " Below page-alignment resolution; folded into shared runtime.",
        "sleep": " Below page-alignment resolution; folded into shared runtime.",
        "sync": " Below page-alignment resolution; folded into shared runtime.",
        "true": " Below page-alignment resolution; folded into shared runtime.",
    }
    return line + extra.get(name, "")


def main() -> None:
    path = Path(__file__).resolve().parents[1] / "docs" / "APPLETS.md"
    text = path.read_text()

    methodology = (
        "### Binary size notes\n\n"
        "Marginal sizes are measured by comparing a stripped release build for "
        "`x86_64-unknown-linux-musl` with all default applets enabled "
        f"(~{FULL:,} bytes) against the same build with that applet disabled. "
        "Values are rounded to the nearest KiB. Because of 4 KiB page alignment and "
        "shared code between applets, marginal costs do not sum exactly to the full "
        "binary size. Applets marked ~0 KiB still add dispatch-table entries but "
        "little or no unique code. Regenerate with "
        "[`scripts/measure-applet-sizes.py`](../scripts/measure-applet-sizes.py) "
        "and [`scripts/patch-applet-sizes-doc.py`](../scripts/patch-applet-sizes-doc.py).\n\n"
    )

    if "### Binary size notes" not in text:
        text = text.replace(
            "(`sh` is an alias for `rash`; `[` is an alias for `test`.)\n\n---",
            "(`sh` is an alias for `rash`; `[` is an alias for `test`.)\n\n"
            + methodology
            + "---",
        )

    text = re.sub(
        r"\n\*\*Approximate binary size\*\* —[^\n]+\n",
        "\n",
        text,
    )

    for name in SIZES:
        heading = f"## `{name}`"
        idx = text.find(heading)
        if idx == -1:
            raise SystemExit(f"missing heading {heading}")
        insert_at = text.find("\n", idx) + 1
        line = size_line(name) + "\n\n"
        text = text[:insert_at] + line + text[insert_at:]

    text = re.sub(
        r"(\*\*Approximate binary size\*\* —[^\n]+\.)\n\n+",
        r"\1\n\n",
        text,
    )

    path.write_text(text)
    print(f"Updated {path}")


if __name__ == "__main__":
    main()
