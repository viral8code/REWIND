#!/usr/bin/env python3
"""Render docs/language-reference.md to docs/language-reference.pdf.

Dependencies: python -m pip install mistune==3.2.0, and Microsoft Edge or Chrome
(the browser is only used for headless printing).
Usage: python scripts/build-reference-pdf.py [--browser PATH]
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import docs_render as dr

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "docs" / "language-reference.md"
OUT = ROOT / "docs" / "language-reference.pdf"


def main():
    browser = dr.find_browser(sys.argv[sys.argv.index("--browser") + 1] if "--browser" in sys.argv else None)
    body, headings = dr.render(SRC.read_text(encoding="utf-8").replace("\r\n", "\n"))
    first = body.index("<h2 ")
    body = body[:first] + dr.toc_html(headings) + body[first:]
    page = ('<!doctype html><html lang="ja"><head><meta charset="utf-8"><title>REWIND 言語リファレンス</title>'
            f"<style>{dr.BASE_CSS}</style></head><body>{body}</body></html>")
    dr.print_pdf(page, OUT, browser)
    print(f"wrote {OUT} ({OUT.stat().st_size} bytes)")


main()
