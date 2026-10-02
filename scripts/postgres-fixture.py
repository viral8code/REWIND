#!/usr/bin/env python3
"""Disposable real PostgreSQL/SCRAM/TLS fixture for Linux and Windows SDK checks.
Keys and the documented test-only password stay in the temporary fixture directory.
"""
import argparse
import glob
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess

PASSWORD = "rewind-fixture-only-password"
USER = "rewind_fixture"


def binary(directory, name):
    return str(directory / (name + (".exe" if os.name == "nt" else "")))


def run(args, **kwargs):
    result = subprocess.run(args, capture_output=True, text=True, **kwargs)
    if result.returncode:
        raise RuntimeError(f"{Path(args[0]).name} failed: {result.stderr}")
    return result


def find_bin(explicit):
    if explicit:
        directory = Path(explicit)
    else:
        candidates = glob.glob("/usr/lib/postgresql/*/bin/initdb") + glob.glob(
            "C:/Program Files/PostgreSQL/*/bin/initdb.exe"
        )
        found = shutil.which("initdb")
        if found:
            candidates.append(found)
        if not candidates:
            raise RuntimeError("Install PostgreSQL server tools or pass --bin")
        directory = Path(sorted(candidates)[-1]).parent
    if not Path(binary(directory, "initdb")).is_file():
        raise RuntimeError("initdb is missing from fixture --bin")
    return directory


def start(args):
    root = Path(args.root).resolve()
    root.mkdir(parents=True, exist_ok=True)
    directory = find_bin(args.bin)
    openssl = shutil.which("openssl") or "C:/Program Files/Git/usr/bin/openssl.exe"
    if not Path(openssl).is_file():
        raise RuntimeError("OpenSSL is needed to generate disposable fixture certificates")
    data = root / "data"
    if data.exists():
        raise RuntimeError("Refusing to replace an existing PostgreSQL fixture")
    (root / "password.txt").write_text(PASSWORD + "\n", encoding="ascii")
    (root / "password.txt").chmod(0o600)
    init = [binary(directory, "initdb"), "-D", str(data), "-U", USER,
            "--no-locale", "-E", "UTF8", "--auth-host=scram-sha-256",
            "--auth-local=trust", "--pwfile=" + str(root / "password.txt")]
    if args.share:
        init += ["-L", args.share]
    run(init)
    run([openssl, "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-keyout",
         str(root / "ca.key"), "-out", str(root / "ca.pem"), "-days", "7", "-subj",
         "/CN=REWIND-disposable-test-root"])
    run([openssl, "req", "-newkey", "rsa:2048", "-nodes", "-keyout",
         str(root / "server.key"), "-out", str(root / "server.csr"), "-subj", "/CN=localhost"])
    (root / "server.ext").write_text(
        "basicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\n"
        "extendedKeyUsage=serverAuth\nsubjectAltName=DNS:localhost,IP:127.0.0.1\n", encoding="ascii")
    run([openssl, "x509", "-req", "-in", str(root / "server.csr"), "-CA",
         str(root / "ca.pem"), "-CAkey", str(root / "ca.key"), "-CAcreateserial",
         "-out", str(root / "server.pem"), "-days", "7", "-extfile", str(root / "server.ext")])
    run([openssl, "x509", "-in", str(root / "ca.pem"), "-outform", "DER", "-out", str(root / "ca.der")])
    (root / "server.key").chmod(0o600)
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        port = sock.getsockname()[1]
    quote = lambda path: str(path).replace("\\", "/").replace("'", "''")
    with (data / "postgresql.conf").open("a", encoding="utf-8") as stream:
        stream.write(f"\nlisten_addresses='127.0.0.1'\nport={port}\nssl=on\n"
                     f"ssl_cert_file='{quote(root / 'server.pem')}'\n"
                     f"ssl_key_file='{quote(root / 'server.key')}'\n"
                     "shared_buffers='16MB'\nmax_connections=24\nstatement_timeout=120000\n")
        stream.write(f"unix_socket_directories='{quote(root) if os.name != 'nt' else ''}'\n")
    run([binary(directory, "pg_ctl"), "-D", str(data), "-l", str(root / "server.log"), "-w", "start"])
    dsn = f"host=127.0.0.1 port={port} user={USER} password={PASSWORD} dbname=postgres sslmode=require"
    env = {"REWIND_TEST_PG_DSN": dsn, "REWIND_TEST_PG_CA": str(root / "ca.der"),
           "REWIND_REQUIRE_PG": "1", "REWIND_PG_FIXTURE": str(root)}
    (root / "fixture.json").write_text(json.dumps({"bin": str(directory), "env": env}), encoding="utf-8")
    if args.github_env:
        with open(args.github_env, "a", encoding="utf-8") as stream:
            for key, value in env.items():
                stream.write(f"{key}={value}\n")
    print(f"Disposable PostgreSQL TLS fixture started on port {port}")


def stop(args):
    root = Path(args.root).resolve()
    config = json.loads((root / "fixture.json").read_text(encoding="utf-8"))
    run([binary(Path(config["bin"]), "pg_ctl"), "-D", str(root / "data"), "-m", "fast", "-w", "stop"])
    print("Disposable PostgreSQL fixture stopped")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("action", choices=["start", "stop"])
    parser.add_argument("--root", required=True)
    parser.add_argument("--bin")
    parser.add_argument("--share")
    parser.add_argument("--github-env")
    args = parser.parse_args()
    (start if args.action == "start" else stop)(args)
