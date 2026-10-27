#!/usr/bin/env python3
"""Build docs/book/index.html and docs/book/REWIND-4.2.0-book.pdf from docs/book/chapters/*.md.

Dependencies: python -m pip install mistune==3.2.0, and Microsoft Edge or Chrome for the PDF.
Check the code examples first: python scripts/check-docs-examples.py docs/book/chapters/*.md
Usage: python scripts/build-book.py [--browser PATH] [--no-pdf]
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import docs_render as dr

ROOT = Path(__file__).resolve().parents[1]
BOOK = ROOT / "docs" / "book"
PDF = BOOK / "REWIND-4.2.0-book.pdf"


def main():
    parts = [p.read_text(encoding="utf-8").replace("\r\n", "\n") for p in sorted((BOOK / "chapters").glob("*.md"))]
    body, headings = dr.render("\n\n".join(parts))
    first = body.index("<h2 ")
    body = body[:first] + dr.toc_html(headings, h3_pattern=None) + body[first:]
    page = ('<!doctype html><html lang="ja"><head><meta charset="utf-8">'
            '<meta name="viewport" content="width=device-width,initial-scale=1">'
            '<title>REWIND 入門</title>'
            f"<style>{dr.BASE_CSS}{dr.SCREEN_CSS}</style></head><body>{body}</body></html>")
    (BOOK / "index.html").write_text(page, encoding="utf-8")
    print(f"wrote {BOOK / 'index.html'}")
    if "--no-pdf" not in sys.argv:
        browser = dr.find_browser(sys.argv[sys.argv.index("--browser") + 1] if "--browser" in sys.argv else None)
        dr.print_pdf(page, PDF, browser)
        print(f"wrote {PDF} ({PDF.stat().st_size} bytes)")


main()
