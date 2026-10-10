#!/usr/bin/env python3
"""Import the book's exact API and sources from a verified v2.0.0 SDK archive."""
import argparse, hashlib, json, re, tarfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("archive", type=Path)
args = parser.parse_args()
with tarfile.open(args.archive) as archive:
    files = {m.name: m for m in archive.getmembers() if m.isfile()}
    def read(suffix):
        matches = [m for name, m in files.items() if name.endswith(suffix)]
        if len(matches) != 1:
            raise ValueError(f"Expected exactly one {suffix}: found {len(matches)}")
        return archive.extractfile(matches[0]).read().decode("utf-8")
    index = json.loads(read("/doc/std-api.json"))
    if index["compiler"] != "2.0.0" or index["language"] != "2.0.0":
        raise ValueError("This book targets the published 2.0.0 SDK")
    modules = {}
    for name in sorted(index["modules"]):
        api = json.loads(read(f"/doc/std/{name}.api.json"))
        markdown = read(f"/doc/std/{name}.md")
        declarations = re.findall(r"^- `(pub .*?)`$", markdown, re.M)
        signatures = {}
        for declaration in declarations:
            match = re.match(r"pub (?:async )?fn (\w+)", declaration)
            if match:
                signatures[match[1]] = declaration
        functions = {key[3:]: value for key,value in api["symbols"].items() if key.startswith("fn:")}
        if set(functions) != set(signatures):
            raise ValueError(f"Signature/API mismatch in {name}")
        source = read(f"/lib/rewind/std/{name}.rw")
        local = (ROOT/"libraries"/"std"/f"{name}.rw").read_text()
        if source != local:
            raise ValueError(f"Published/local source differs: {name}")
        modules[name] = {
            "functions": functions, "signatures": signatures,
            "symbols": {k:v for k,v in api["symbols"].items() if not k.startswith("fn:")},
            "declarations": [d for d in declarations if not re.match(r"pub (?:async )?fn ",d)],
            "source_sha256": hashlib.sha256(source.encode()).hexdigest()
        }
    common = dict(next(iter(modules.values()))["symbols"])
    for module in modules.values():
        common = {k:v for k,v in common.items() if module["symbols"].get(k)==v}
    for module in modules.values():
        module["symbols"] = {k:v for k,v in module["symbols"].items() if k not in common}
data = {
    "compiler": "2.0.0", "language": "2.0.0",
    "implementation_commit": "d0f3ee6684769d3f92dfb0e21c55deffc00dc181",
    "sdk_archive_sha256": hashlib.sha256(args.archive.read_bytes()).hexdigest(),
    "common_symbols": common, "modules": modules
}
out=ROOT/"docs"/"book"/"api-snapshot.json"
out.write_text(json.dumps(data,ensure_ascii=False,indent=2)+"\n")
print(f"{len(modules)} modules, {sum(len(m['functions']) for m in modules.values())} functions, {len(common)} common types; {out}")
