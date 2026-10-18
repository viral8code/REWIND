#!/usr/bin/env python3
"""Render docs/language-reference.md to docs/language-reference.pdf.

Dependencies: python -m pip install mistune==3.2.0, and Microsoft Edge or Chrome
(the browser is only used for headless printing).
Usage: python scripts/build-reference-pdf.py [--browser PATH]
"""
import html, os, re, subprocess, sys, tempfile
from pathlib import Path
import mistune

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "docs" / "language-reference.md"
OUT = ROOT / "docs" / "language-reference.pdf"
CANDIDATES = [
    r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
    r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
    r"C:\Program Files\Google\Chrome\Application\chrome.exe",
    "/usr/bin/chromium", "/usr/bin/chromium-browser", "/usr/bin/google-chrome",
]

def slug(text):
    text = html.unescape(re.sub(r"<[^>]+>", "", text)).strip().lower()
    text = re.sub(r"[^\w\s\-]", "", text)
    return re.sub(r"\s", "-", text)

headings = []

class Renderer(mistune.HTMLRenderer):
    def heading(self, text, level, **attrs):
        ident = slug(text)
        headings.append((level, ident, html.unescape(re.sub(r"<[^>]+>", "", text))))
        return f'<h{level} id="{ident}">{text}</h{level}>\n'

    def block_code(self, code, info=None):
        tag = (info or "").split(" ")[0]
        label = ""
        if tag.startswith("rewind-error"):
            label = '<div class="cap">エラーになる例</div>'
        elif tag.startswith("rewind"):
            label = '<div class="cap">REWIND</div>'
        elif tag == "text":
            label = '<div class="cap out">出力</div>'
        return f'<div class="code">{label}<pre><code>{html.escape(code.rstrip())}</code></pre></div>\n'

CSS = """
@page { size: A4; margin: 20mm 17mm 22mm 17mm;
  @bottom-center { content: counter(page); font: 9pt 'Yu Gothic UI','Meiryo',sans-serif; color: #666; } }
:root { color-scheme: light; }
body { font: 10.3pt/1.85 'Yu Gothic UI','Meiryo','Noto Sans CJK JP',sans-serif; color: #1c2024; margin: 0; }
h1 { font-size: 24pt; margin: 0 0 6mm; border-bottom: 2px solid #2a5db0; padding-bottom: 3mm; }
h2 { font-size: 16pt; margin: 12mm 0 3mm; padding-left: 3mm; border-left: 4px solid #2a5db0; break-before: page; break-after: avoid; }
h2.nobreak { break-before: auto; }
h3 { font-size: 12.5pt; margin: 7mm 0 2mm; break-after: avoid; color: #1f4a8f; }
h4 { font-size: 11pt; margin: 5mm 0 1.5mm; break-after: avoid; }
p, li { orphans: 3; widows: 3; }
a { color: #1f4a8f; text-decoration: none; }
code { font: 9.2pt Consolas,'BIZ UDGothic','MS Gothic',monospace; background: #eef1f6; padding: 0 1.2mm; border-radius: 2px; }
.code { margin: 3mm 0; border: 1px solid #ccd3df; border-radius: 3px; background: #f6f8fb; break-inside: avoid-page; }
.code pre { margin: 0; font: 8.8pt/1.5 Consolas,'BIZ UDGothic','MS Gothic',monospace; padding: 2.5mm 3.5mm; white-space: pre-wrap; word-break: break-all; }
.code code { background: none; padding: 0; font: inherit; line-height: inherit; }
.cap { font: 7.5pt sans-serif; color: #fff; background: #5d7396; padding: 0.4mm 3mm; letter-spacing: .08em; }
.cap.out { background: #7a8a6a; }
table { border-collapse: collapse; margin: 3mm 0; font-size: 9.3pt; line-height: 1.55; }
th, td { border: 1px solid #c4ccd8; padding: 1.2mm 2.6mm; vertical-align: top; }
th { background: #e8edf5; }
tr { break-inside: avoid; }
blockquote { margin: 3mm 0; padding: 1mm 4mm; border-left: 3px solid #b6c3da; background: #f4f6fa; color: #333; }
.toc { break-after: page; }
.toc ol { list-style: none; padding-left: 0; margin: 0; }
.toc li { margin: 0; line-height: 1.7; }
.toc li.l3 { padding-left: 7mm; font-size: 9pt; color: #444; }
.toc li.l2 { margin-top: 1.5mm; font-weight: bold; }
.toc h2 { break-before: auto; }
"""

def main():
    md = SRC.read_text(encoding="utf-8").replace("\r\n", "\n")
    body = mistune.create_markdown(renderer=Renderer(escape=False), plugins=["table", "strikethrough"])(md)
    toc = ['<nav class="toc"><h2 class="nobreak" id="toc">目次</h2><ol>']
    for level, ident, text in headings[1:]:
        if level == 2:
            toc.append(f'<li class="l2"><a href="#{ident}">{html.escape(text)}</a></li>')
        elif level == 3 and re.match(r"\d+\.\d+ ", text):
            toc.append(f'<li class="l3"><a href="#{ident}">{html.escape(text)}</a></li>')
    toc.append("</ol></nav>")
    # put the table of contents after the title block (title + preface before the first h2)
    first = body.index("<h2 ")
    body = body[:first] + "".join(toc) + body[first:]
    page = f'<!doctype html><html lang="ja"><head><meta charset="utf-8"><title>REWIND 言語リファレンス</title><style>{CSS}</style></head><body>{body}</body></html>'
    browser = os.environ.get("BROWSER")
    if "--browser" in sys.argv:
        browser = sys.argv[sys.argv.index("--browser") + 1]
    browser = browser or next((c for c in CANDIDATES if Path(c).exists()), None)
    if not browser:
        raise SystemExit("no Edge/Chrome found; pass --browser PATH")
    with tempfile.TemporaryDirectory() as tmp:
        src = Path(tmp) / "reference.html"
        src.write_text(page, encoding="utf-8")
        subprocess.run([browser, "--headless=new", "--disable-gpu", "--no-pdf-header-footer",
                        "--generate-pdf-document-outline", f"--user-data-dir={tmp}/profile",
                        f"--print-to-pdf={OUT}", src.as_uri()], check=True, timeout=180)
    print(f"wrote {OUT} ({OUT.stat().st_size} bytes)")

main()
