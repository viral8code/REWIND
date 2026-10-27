#!/usr/bin/env python3
"""Run the code examples of the Markdown documents and compare them with the text.

Usage: python scripts/check-docs-examples.py [--rewind PATH] FILE.md [FILE.md ...]

Fences understood (the first word of the info string selects the kind):

  ```rewind [options]            complete program; must exit 0.
                                 A directly following ```text block is the expected stdout.
  ```rewind-error TOKEN          must exit non-zero; stderr must contain TOKEN.
  ```rewind-exit N [options]     must exit with code N; stdout is compared like `rewind`.

Options (name=value, value may be "quoted"; \\n inside stdin is a newline):

  allow=EFFECTS   --allow-effects list (default fileRead,fileWrite,clock,external)
  stdin=TEXT      text sent to the standard input
  args=A,B        program arguments (after --)
  steps=N         --steps N
  gui=FILE        --gui-events docs/book/fixtures/FILE
  http=1          start a local HTTP server and pass its URL as the only argument

All other fences (sh, text, toml, unlabeled) are not executed.
"""
import http.server
import os
import re
import subprocess
import sys
import tempfile
import threading
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "docs" / "book" / "fixtures"


class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        body = b"hello from server"
        self.send_response(200)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *args):
        pass


def parse_opts(text):
    out = {}
    for match in re.finditer(r'(\w+)=("[^"]*"|\S+)', text):
        value = match.group(2)
        out[match.group(1)] = value[1:-1] if value.startswith('"') else value
    return out


def fences(text):
    blocks, current = [], None
    for line in text.split("\n"):
        if current is None:
            if line.startswith("```"):
                current = [line[3:].strip(), []]
        elif line.rstrip() == "```":
            blocks.append((current[0], "\n".join(current[1]) + "\n"))
            current = None
        else:
            current[1].append(line)
    if current is not None:
        raise SystemExit("unclosed code fence")
    return blocks


def main():
    args = sys.argv[1:]
    exe = ROOT / "target" / "release" / ("rewind.exe" if os.name == "nt" else "rewind")
    if "--rewind" in args:
        i = args.index("--rewind")
        exe = Path(args[i + 1])
        del args[i:i + 2]
    if not args:
        raise SystemExit(__doc__)
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    total = failed = 0
    for name in args:
        text = Path(name).read_text(encoding="utf-8").replace("\r\n", "\n")
        blocks = fences(text)
        file_total = file_failed = 0
        for index, (tag, body) in enumerate(blocks):
            if not tag.startswith("rewind"):
                continue
            kind = tag.split()[0]
            rest = tag[len(kind):].strip()
            expect_code = 0
            token = ""
            if kind == "rewind-error":
                token, opts = rest, {}
            elif kind == "rewind-exit":
                code_text, _, option_text = rest.partition(" ")
                expect_code, opts = int(code_text), parse_opts(option_text)
            elif kind == "rewind":
                opts = parse_opts(rest)
            else:
                raise SystemExit(f"{name}: unknown fence {tag!r}")
            file_total += 1
            flags = ["--allow-effects", opts.get("allow", "fileRead,fileWrite,clock,external")]
            if "steps" in opts:
                flags += ["--steps", opts["steps"]]
            if "gui" in opts:
                flags += ["--gui-events", str(FIXTURES / opts["gui"])]
            tail = []
            if "args" in opts:
                tail = ["--"] + opts["args"].split(",")
            if "http" in opts:
                tail = ["--", f"http://127.0.0.1:{server.server_port}/hi"]
            stdin = opts.get("stdin", "").replace("\\n", "\n").encode("utf-8")
            with tempfile.TemporaryDirectory(prefix="docs-example-") as directory:
                (Path(directory) / "main.rw").write_text(body, encoding="utf-8")
                done = subprocess.run([str(exe), "run", "main.rw", *flags, *tail], cwd=directory,
                                      capture_output=True, timeout=180, input=stdin)
            out = done.stdout.decode("utf-8", "replace").replace("\r\n", "\n")
            err = done.stderr.decode("utf-8", "replace")
            problem = None
            if kind == "rewind-error":
                if done.returncode == 0:
                    problem = "expected a failure but the program succeeded"
                elif token and token not in err:
                    problem = f"stderr does not contain {token!r}:\n{err[:300]}"
            elif done.returncode != expect_code:
                problem = f"exit code {done.returncode}, expected {expect_code}:\n{err[:400]}"
            if problem is None and kind != "rewind-error" and index + 1 < len(blocks) and blocks[index + 1][0] == "text":
                expected = blocks[index + 1][1]
                if out.rstrip("\n") != expected.rstrip("\n"):
                    problem = f"stdout differs\n--- expected\n{expected}--- actual\n{out}"
            if problem:
                file_failed += 1
                print(f"{name}: example #{file_total} [{tag}] FAILED: {problem}\n{body}")
        print(f"{name}: {file_total} examples, {file_failed} failed")
        total += file_total
        failed += file_failed
    server.shutdown()
    print(f"total {total} examples, {failed} failed")
    sys.exit(1 if failed else 0)


main()
