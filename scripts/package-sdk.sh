#!/usr/bin/env bash
# Assemble and verify release assets without publishing them.
set -euo pipefail
REPO_ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$REPO_ROOT"
REWIND_BINARY=${REWIND_BINARY:-"$REPO_ROOT/target/release/rewind"}
RELEASE_OUTPUT=${1:?usage: package-sdk.sh NEW_OUTPUT_DIRECTORY}
VERSION=$(sed -n 's/^version = "\([^"]*\)"$/\1/p' Cargo.toml | head -n 1)
COMMIT=$(git rev-parse HEAD)
ARCHIVE_EPOCH=$(git show -s --format=%ct HEAD)
[[ -z $(git status --porcelain --untracked-files=no) ]] || { echo 'Tracked files must be clean' >&2; exit 1; }
mkdir "$RELEASE_OUTPUT"
RELEASE_OUTPUT=$(cd "$RELEASE_OUTPUT" && pwd)
RELEASE_WORK=$(mktemp -d)
trap 'rm -rf "$RELEASE_WORK"' EXIT
umask 077
PUBLIC_KEY=$("$REWIND_BINARY" keygen "$RELEASE_WORK/signing-seed")
SDK_NAME="rewind-$VERSION-linux-x86_64"
"$REWIND_BINARY" sdk-build --output "$RELEASE_WORK/$SDK_NAME" --key "$RELEASE_WORK/signing-seed"
"$REWIND_BINARY" sdk-verify --sdk "$RELEASE_WORK/$SDK_NAME" --public-key "$PUBLIC_KEY"
# Stable public read/execute permissions, independent of the builder's umask.
python3 - "$RELEASE_WORK/$SDK_NAME" <<'PYMODES'
import pathlib, sys
root=pathlib.Path(sys.argv[1])
root.chmod(0o755)
for path in root.rglob('*'):
    if path.is_dir(): path.chmod(0o755)
    else: path.chmod(0o755 if path.parent == root/'bin' else 0o644)
PYMODES
for BASELINE in libraries/api/*.api.json; do
  "$REWIND_BINARY" api-diff "$BASELINE" "$RELEASE_WORK/$SDK_NAME/share/rewind/doc/std/$(basename "$BASELINE")" --deny-breaking > /dev/null
done
# The private seed stays outside all distribution directories.
tar --sort=name --mtime="@$ARCHIVE_EPOCH" --owner=0 --group=0 --numeric-owner -cf - -C "$RELEASE_WORK" "$SDK_NAME" | gzip -n > "$RELEASE_OUTPUT/$SDK_NAME.tar.gz"
git archive --format=tar.gz --prefix="rewind-$VERSION-source/" --output="$RELEASE_OUTPUT/rewind-$VERSION-source.tar.gz" HEAD
printf '%s\n' "$PUBLIC_KEY" > "$RELEASE_OUTPUT/rewind-$VERSION-sdk.pub"
cp docs/getting-started.md "$RELEASE_OUTPUT/GETTING_STARTED.md"
python3 - "$REWIND_BINARY" "$RELEASE_OUTPUT/BUILD_INFO.json" "$VERSION" "$COMMIT" "$PUBLIC_KEY" <<'PY'
import json, re, subprocess, sys
binary, output, version, commit, public = sys.argv[1:]
symbols = subprocess.check_output(['objdump', '-T', binary], text=True)
versions = re.findall(r'GLIBC_(\d+\.\d+)', symbols)
minimum = max(versions, key=lambda s: tuple(map(int, s.split('.'))))
with open(output, 'w') as f:
    json.dump(dict(version=version, commit=commit, target='x86_64-unknown-linux-gnu', minimum_glibc=minimum, sdk_public_key=public, signing_key_scope='this release only', rust_toolchain='1.98.1', build_command='cargo build --release --locked'), f, indent=2)
    f.write('\n')
PY
# Smoke-test the actual archive after extraction, including recorded input.
mkdir "$RELEASE_WORK/extracted"
tar -xzf "$RELEASE_OUTPUT/$SDK_NAME.tar.gz" -C "$RELEASE_WORK/extracted"
EXTRACTED="$RELEASE_WORK/extracted/$SDK_NAME"
"$EXTRACTED/bin/rewind" sdk-verify --sdk "$EXTRACTED" --public-key "$PUBLIC_KEY"
cp -R "$EXTRACTED/share/rewind/examples/shortest" "$RELEASE_WORK/shortest"
"$EXTRACTED/bin/rewind" sdk-install --root "$RELEASE_WORK/shortest" --sdk "$EXTRACTED" --public-key "$PUBLIC_KEY"
printf '0\n2\n1\nunreachable\n' > "$RELEASE_WORK/expected"
printf '4 3\n0 1 4\n0 2 1\n2 1 1\n' | "$EXTRACTED/bin/rewind" run --root "$RELEASE_WORK/shortest" --task-steps 2000000 --record "$RELEASE_WORK/trace.json" > "$RELEASE_WORK/actual"
cmp "$RELEASE_WORK/expected" "$RELEASE_WORK/actual"
"$EXTRACTED/bin/rewind" replay "$RELEASE_WORK/trace.json" --root "$RELEASE_WORK/shortest" > "$RELEASE_WORK/replayed"
cmp "$RELEASE_WORK/expected" "$RELEASE_WORK/replayed"
"$EXTRACTED/bin/rewind" build --root "$RELEASE_WORK/shortest" --output "$RELEASE_WORK/app.json"
"$EXTRACTED/bin/rewindc" "$RELEASE_WORK/shortest/main.rw"
rm "$RELEASE_WORK/shortest/main.rw"
rm -rf "$RELEASE_WORK/shortest/vendor"
printf '4 3\n0 1 4\n0 2 1\n2 1 1\n' | "$EXTRACTED/bin/rewind" run-artifact "$RELEASE_WORK/app.json" --root "$RELEASE_WORK/shortest" --task-steps 2000000 --allow-effects input,output > "$RELEASE_WORK/artifact-output"
cmp "$RELEASE_WORK/expected" "$RELEASE_WORK/artifact-output"
printf '4 3\n0 1 4\n0 2 1\n2 1 1\n' | "$EXTRACTED/bin/rewind" run "$RELEASE_WORK/shortest/main.rwc" --task-steps 2000000 --record "$RELEASE_WORK/shortest/compiled-trace.json" --record-mode compact > "$RELEASE_WORK/compiled-project-output"
cmp "$RELEASE_WORK/expected" "$RELEASE_WORK/compiled-project-output"
"$EXTRACTED/bin/rewind" replay "$RELEASE_WORK/shortest/compiled-trace.json" --root "$RELEASE_WORK/shortest" > "$RELEASE_WORK/compiled-project-replay"
cmp "$RELEASE_WORK/expected" "$RELEASE_WORK/compiled-project-replay"

# Test the short commands from the extracted SDK in a manifest-free directory.
mkdir "$RELEASE_WORK/checkpoint"
cp "$EXTRACTED/share/rewind/examples/checkpoint/main.rw" "$RELEASE_WORK/checkpoint/main.rw"
printf '99\n10\n' > "$RELEASE_WORK/checkpoint-expected"
"$EXTRACTED/bin/rewind" run "$RELEASE_WORK/checkpoint/main.rw" > "$RELEASE_WORK/checkpoint-actual"
cmp "$RELEASE_WORK/checkpoint-expected" "$RELEASE_WORK/checkpoint-actual"
"$EXTRACTED/bin/rewindc" "$RELEASE_WORK/checkpoint/main.rw"
rm "$RELEASE_WORK/checkpoint/main.rw"
rm -rf "$RELEASE_WORK/checkpoint/.rewind"
"$EXTRACTED/bin/rewind" "$RELEASE_WORK/checkpoint/main.rwc" > "$RELEASE_WORK/checkpoint-artifact"
cmp "$RELEASE_WORK/checkpoint-expected" "$RELEASE_WORK/checkpoint-artifact"
# Verify the new syntax and conditional restore from the actual shipped SDK.
mkdir "$RELEASE_WORK/conditional"
cp "$EXTRACTED/share/rewind/examples/conditional-revert/main.rw" "$RELEASE_WORK/conditional/main.rw"
printf 'Odd\n' > "$RELEASE_WORK/conditional-expected"
"$EXTRACTED/bin/rewind" run "$RELEASE_WORK/conditional/main.rw" > "$RELEASE_WORK/conditional-actual"
cmp "$RELEASE_WORK/conditional-expected" "$RELEASE_WORK/conditional-actual"
"$EXTRACTED/bin/rewindc" "$RELEASE_WORK/conditional/main.rw"
rm "$RELEASE_WORK/conditional/main.rw"
rm -rf "$RELEASE_WORK/conditional/.rewind"
"$EXTRACTED/bin/rewind" "$RELEASE_WORK/conditional/main.rwc" > "$RELEASE_WORK/conditional-artifact"
cmp "$RELEASE_WORK/conditional-expected" "$RELEASE_WORK/conditional-artifact"
mkdir "$RELEASE_WORK/gui"
cp "$EXTRACTED/share/rewind/examples/gui/main.rw" "$RELEASE_WORK/gui/main.rw"
cp "$EXTRACTED/share/rewind/examples/gui/events.json" "$RELEASE_WORK/gui/events.json"
printf '1\n' > "$RELEASE_WORK/gui-expected"
"$EXTRACTED/bin/rewind" run "$RELEASE_WORK/gui/main.rw" --allow-effects gui --gui-events "$RELEASE_WORK/gui/events.json" --record "$RELEASE_WORK/gui/trace.json" > "$RELEASE_WORK/gui-actual"
cmp "$RELEASE_WORK/gui-expected" "$RELEASE_WORK/gui-actual"
"$EXTRACTED/bin/rewind" replay "$RELEASE_WORK/gui/trace.json" --root "$RELEASE_WORK/gui" --allow-effects gui > "$RELEASE_WORK/gui-replayed"
cmp "$RELEASE_WORK/gui-expected" "$RELEASE_WORK/gui-replayed"
"$EXTRACTED/bin/rewindc" "$RELEASE_WORK/gui/main.rw" --allow-effects gui
rm "$RELEASE_WORK/gui/main.rw"
rm -rf "$RELEASE_WORK/gui/.rewind"
"$EXTRACTED/bin/rewind" "$RELEASE_WORK/gui/main.rwc" --allow-effects gui --gui-events "$RELEASE_WORK/gui/events.json" > "$RELEASE_WORK/gui-artifact"
cmp "$RELEASE_WORK/gui-expected" "$RELEASE_WORK/gui-artifact"
mkdir "$RELEASE_WORK/notes"
cp "$EXTRACTED/share/rewind/examples/notes/main.rw" "$RELEASE_WORK/notes/main.rw"
cp "$EXTRACTED/share/rewind/examples/notes/events.json" "$RELEASE_WORK/notes/events.json"
"$EXTRACTED/bin/rewind" run "$RELEASE_WORK/notes/main.rw" --allow-effects gui,fileRead,fileWrite --gui-events "$RELEASE_WORK/notes/events.json" --record "$RELEASE_WORK/notes/trace.json"
printf '日本語\nnotes' > "$RELEASE_WORK/notes-expected"
cmp "$RELEASE_WORK/notes-expected" "$RELEASE_WORK/notes/notes.txt"
"$EXTRACTED/bin/rewind" replay "$RELEASE_WORK/notes/trace.json" --root "$RELEASE_WORK/notes" --allow-effects gui,fileRead,fileWrite
"$EXTRACTED/bin/rewindc" "$RELEASE_WORK/notes/main.rw" --allow-effects gui,fileRead,fileWrite
rm "$RELEASE_WORK/notes/main.rw" "$RELEASE_WORK/notes/notes.txt"
rm -rf "$RELEASE_WORK/notes/.rewind"
"$EXTRACTED/bin/rewind" "$RELEASE_WORK/notes/main.rwc" --allow-effects gui,fileRead,fileWrite --gui-events "$RELEASE_WORK/notes/events.json"
cmp "$RELEASE_WORK/notes-expected" "$RELEASE_WORK/notes/notes.txt"
cp -R "$EXTRACTED/share/rewind/examples/external" "$RELEASE_WORK/external"
"$EXTRACTED/bin/rewind" run "$RELEASE_WORK/external/main.rw" --allow-effects external,clock --record "$RELEASE_WORK/external/trace.json" > "$RELEASE_WORK/external/actual"
python3 - "$RELEASE_WORK/external/actual" <<'PYCLOCK'
import pathlib,sys
lines=pathlib.Path(sys.argv[1]).read_text().splitlines()
assert len(lines)==2 and lines[0]==lines[1] and int(lines[0])>0
PYCLOCK
"$EXTRACTED/bin/rewind" replay "$RELEASE_WORK/external/trace.json" --root "$RELEASE_WORK/external" --allow-effects external,clock > "$RELEASE_WORK/external/replayed"
cmp "$RELEASE_WORK/external/actual" "$RELEASE_WORK/external/replayed"
"$EXTRACTED/bin/rewindc" "$RELEASE_WORK/external/main.rw" --allow-effects external,clock
rm "$RELEASE_WORK/external/main.rw"
rm -rf "$RELEASE_WORK/external/.rewind"
"$EXTRACTED/bin/rewind" "$RELEASE_WORK/external/main.rwc" --allow-effects external,clock > "$RELEASE_WORK/external/compiled"
python3 - "$RELEASE_WORK/external/compiled" <<'PYCLOCK'
import pathlib,sys
lines=pathlib.Path(sys.argv[1]).read_text().splitlines()
assert len(lines)==2 and lines[0]==lines[1] and int(lines[0])>0
PYCLOCK
python3 scripts/smoke-http-sdk.py "$EXTRACTED/bin/rewind" "$RELEASE_WORK/http"
python3 scripts/smoke-http-pool-sdk.py "$EXTRACTED/bin/rewind" "$RELEASE_WORK/http-pool"
python3 scripts/smoke-text-sdk.py "$EXTRACTED/bin/rewind" "$RELEASE_WORK/text-storage"
python3 scripts/smoke-numeric-digests-sdk.py "$EXTRACTED/bin/rewind" "$RELEASE_WORK/numeric-digests"
python3 scripts/smoke-http-stream-sdk.py "$EXTRACTED/bin/rewind" "$RELEASE_WORK/http-stream"
python3 scripts/smoke-numeric-sdk.py "$EXTRACTED/bin/rewind" "$RELEASE_WORK/numeric"
python3 scripts/smoke-tcp-tls-sdk.py "$EXTRACTED/bin/rewind" "$RELEASE_WORK/tcp-tls"
python3 scripts/smoke-fft-sdk.py "$EXTRACTED/bin/rewind" "$RELEASE_WORK/fft-async"
python3 scripts/smoke-sparse-sdk.py "$EXTRACTED/bin/rewind" "$RELEASE_WORK/sparse-async"
python3 scripts/smoke-vector-sdk.py "$EXTRACTED/bin/rewind" "$RELEASE_WORK/vector-async"
python3 scripts/smoke-solve-sdk.py "$EXTRACTED/bin/rewind" "$RELEASE_WORK/solve-async"
python3 scripts/smoke-qr-sdk.py "$EXTRACTED/bin/rewind" "$RELEASE_WORK/qr-async"
python3 scripts/smoke-least-squares-sdk.py "$EXTRACTED/bin/rewind" "$RELEASE_WORK/least-squares-async"
python3 scripts/smoke-eigen-sdk.py "$EXTRACTED/bin/rewind" "$RELEASE_WORK/eigen-async"
python3 scripts/smoke-http-server-auth-sdk.py "$EXTRACTED/bin/rewind" "$RELEASE_WORK/http-server-auth"
python3 scripts/smoke-http-server-tls-sdk.py "$EXTRACTED/bin/rewind" "$RELEASE_WORK/http-server-tls"
python3 scripts/smoke-http-server-sdk.py "$EXTRACTED/bin/rewind" "$RELEASE_WORK/http-server"
python3 scripts/smoke-tcp-sdk.py "$EXTRACTED/bin/rewind" "$RELEASE_WORK/tcp"
python3 scripts/smoke-live-sdk.py "$EXTRACTED/bin/rewind" "$RELEASE_WORK/live"
python3 scripts/smoke-gui-async-sdk.py "$EXTRACTED/bin/rewind" "$RELEASE_WORK/gui-async"
python3 scripts/smoke-gui-data-sdk.py "$EXTRACTED/bin/rewind" "$RELEASE_WORK/gui-data"
python3 scripts/smoke-service-data-sdk.py "$EXTRACTED/bin/rewind" "$RELEASE_WORK/service-data"
python3 scripts/smoke-gui-async-sdk.py "$EXTRACTED/bin/rewind" "$RELEASE_WORK/gui-live" --live
python3 scripts/smoke-db-sdk.py "$EXTRACTED/bin/rewind" "$RELEASE_WORK/database"
python3 scripts/smoke-postgres-sdk.py "$EXTRACTED/bin/rewind" "$RELEASE_WORK/postgres"
"$EXTRACTED/bin/rewind" compile --help > /dev/null
(cd "$RELEASE_OUTPUT" && sha256sum ./*.tar.gz ./rewind-*-sdk.pub ./GETTING_STARTED.md ./BUILD_INFO.json > SHA256SUMS)
chmod 644 "$RELEASE_OUTPUT"/*
echo "Verified release assets: $RELEASE_OUTPUT"
