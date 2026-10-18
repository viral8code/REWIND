"""Shared Markdown -> HTML -> PDF rendering for the REWIND documents.

Dependencies: python -m pip install mistune==3.2.0
The PDF step prints the HTML with Microsoft Edge or Chrome (headless).
"""
import html
import os
import re
import subprocess
import tempfile
from pathlib import Path

import mistune

CANDIDATES = [
    r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
    r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
    r"C:\Program Files\Google\Chrome\Application\chrome.exe",
    "/usr/bin/chromium",
    "/usr/bin/chromium-browser",
    "/usr/bin/google-chrome",
]


def slug(text):
    text = html.unescape(re.sub(r"<[^>]+>", "", text)).strip().lower()
    text = re.sub(r"[^\w\s\-]", "", text)
    return re.sub(r"\s", "-", text)


def render(markdown_text):
    """Return (body_html, headings); headings are (level, id, plain text)."""
    headings = []
    used = {}

    class Renderer(mistune.HTMLRenderer):
        def heading(self, text, level, **attrs):
            ident = slug(text)
            count = used.get(ident, 0)
            used[ident] = count + 1
            if count:
                ident = f"{ident}-{count + 1}"
            headings.append((level, ident, html.unescape(re.sub(r"<[^>]+>", "", text))))
            return f'<h{level} id="{ident}">{text}</h{level}>\n'

        def block_code(self, code, info=None):
            tag = (info or "").split(" ")[0]
            label = ""
            if tag.startswith("rewind-error"):
                label = '<div class="cap err">エラーになる例</div>'
            elif tag.startswith("rewind"):
                label = '<div class="cap">REWIND</div>'
            elif tag == "text":
                label = '<div class="cap out">出力</div>'
            elif tag in ("sh", "powershell"):
                label = '<div class="cap sh">コマンド</div>'
            return f'<div class="code">{label}<pre><code>{html.escape(code.rstrip())}</code></pre></div>\n'

    body = mistune.create_markdown(renderer=Renderer(escape=False), plugins=["table", "strikethrough"])(markdown_text)
    return body, headings


BASE_CSS = """
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
.code pre { margin: 0; padding: 2.5mm 3.5mm; font: 8.8pt/1.5 Consolas,'BIZ UDGothic','MS Gothic',monospace; white-space: pre-wrap; word-break: break-all; }
.code code { background: none; padding: 0; font: inherit; line-height: inherit; }
.cap { font: 7.5pt sans-serif; color: #fff; background: #5d7396; padding: 0.4mm 3mm; letter-spacing: .08em; }
.cap.out { background: #7a8a6a; }
.cap.err { background: #a15a5a; }
.cap.sh { background: #6b6b6b; }
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
@page { size: A4; margin: 20mm 17mm 22mm 17mm;
  @bottom-center { content: counter(page); font: 9pt 'Yu Gothic UI','Meiryo',sans-serif; color: #666; } }
"""

SCREEN_CSS = """
@media screen {
  body { max-width: 860px; margin: 0 auto; padding: 24px 16px 80px; background: #fff; }
  h2 { break-before: auto; margin-top: 56px; }
  .toc { break-after: auto; }
}
"""


def toc_html(headings, skip_first=True, h3_pattern=r"\d+\.\d+ "):
    items = ['<nav class="toc"><h2 class="nobreak" id="toc">目次</h2><ol>']
    for level, ident, text in headings[1 if skip_first else 0:]:
        if level == 2:
            items.append(f'<li class="l2"><a href="#{ident}">{html.escape(text)}</a></li>')
        elif level == 3 and h3_pattern and re.match(h3_pattern, text):
            items.append(f'<li class="l3"><a href="#{ident}">{html.escape(text)}</a></li>')
        elif level == 3 and not h3_pattern:
            items.append(f'<li class="l3"><a href="#{ident}">{html.escape(text)}</a></li>')
    items.append("</ol></nav>")
    return "".join(items)


def find_browser(explicit=None):
    explicit = explicit or os.environ.get("BROWSER")
    browser = explicit or next((c for c in CANDIDATES if Path(c).exists()), None)
    if not browser:
        raise SystemExit("no Edge/Chrome found; pass --browser PATH")
    return browser


def print_pdf(page_html, out_path, browser):
    with tempfile.TemporaryDirectory() as tmp:
        src = Path(tmp) / "page.html"
        src.write_text(page_html, encoding="utf-8")
        subprocess.run(
            [browser, "--headless=new", "--disable-gpu", "--no-pdf-header-footer",
             "--generate-pdf-document-outline", f"--user-data-dir={tmp}/profile",
             f"--print-to-pdf={out_path}", src.as_uri()],
            check=True, timeout=240)
