use super::*;
use serde_json::{json, Value as J};
const MAX_BYTES: u64 = 256 * 1024 * 1024;
const MAX_FILES: usize = 512;
fn invalid(s: &str) -> Error {
    Error::InvalidOperation(format!("SDK: {s}"))
}
fn target() -> String {
    format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS)
}
pub(in crate::v2) fn modules() -> BTreeMap<&'static str, &'static str> {
    BTreeMap::from([
        ("models", include_str!("../../../libraries/std/models.rw")),
        (
            "optimize",
            include_str!("../../../libraries/std/optimize.rw"),
        ),
        (
            "autodiff",
            include_str!("../../../libraries/std/autodiff.rw"),
        ),
        (
            "fftAsync",
            include_str!("../../../libraries/std/fftAsync.rw"),
        ),
        ("fft", include_str!("../../../libraries/std/fft.rw")),
        ("sparse", include_str!("../../../libraries/std/sparse.rw")),
        (
            "sparseAsync",
            include_str!("../../../libraries/std/sparseAsync.rw"),
        ),
        (
            "lazySegment",
            include_str!("../../../libraries/std/lazySegment.rw"),
        ),
        ("trie", include_str!("../../../libraries/std/trie.rw")),
        ("suffix", include_str!("../../../libraries/std/suffix.rw")),
        (
            "geometry",
            include_str!("../../../libraries/std/geometry.rw"),
        ),
        ("flow", include_str!("../../../libraries/std/flow.rw")),
        (
            "matching",
            include_str!("../../../libraries/std/matching.rw"),
        ),
        (
            "distributions",
            include_str!("../../../libraries/std/distributions.rw"),
        ),
        (
            "dbDecimal",
            include_str!("../../../libraries/std/dbDecimal.rw"),
        ),
        (
            "dbDatetime",
            include_str!("../../../libraries/std/dbDatetime.rw"),
        ),
        (
            "datetime",
            include_str!("../../../libraries/std/datetime.rw"),
        ),
        ("clock", include_str!("../../../libraries/std/clock.rw")),
        ("regex", include_str!("../../../libraries/std/regex.rw")),
        (
            "jsonStream",
            include_str!("../../../libraries/std/jsonStream.rw"),
        ),
        (
            "csvStream",
            include_str!("../../../libraries/std/csvStream.rw"),
        ),
        ("unicode", include_str!("../../../libraries/std/unicode.rw")),
        ("decimal", include_str!("../../../libraries/std/decimal.rw")),
        ("bigint", include_str!("../../../libraries/std/bigint.rw")),
        ("error", include_str!("../../../libraries/std/error.rw")),
        (
            "stringSearch",
            include_str!("../../../libraries/std/stringSearch.rw"),
        ),
        ("bitset", include_str!("../../../libraries/std/bitset.rw")),
        ("range", include_str!("../../../libraries/std/range.rw")),
        (
            "rollbackSet",
            include_str!("../../../libraries/std/rollbackSet.rw"),
        ),
        ("matrix", include_str!("../../../libraries/std/matrix.rw")),
        ("dp", include_str!("../../../libraries/std/dp.rw")),
        ("csv", include_str!("../../../libraries/std/csv.rw")),
        (
            "external",
            include_str!("../../../libraries/std/external.rw"),
        ),
        (
            "httpServer",
            include_str!("../../../libraries/std/httpServer.rw"),
        ),
        (
            "httpRouter",
            include_str!("../../../libraries/std/httpRouter.rw"),
        ),
        ("tcp", include_str!("../../../libraries/std/tcp.rw")),
        ("http", include_str!("../../../libraries/std/http.rw")),
        ("db", include_str!("../../../libraries/std/db.rw")),
        ("gui", include_str!("../../../libraries/std/gui.rw")),
        ("guiForm", include_str!("../../../libraries/std/guiForm.rw")),
        (
            "guiTable",
            include_str!("../../../libraries/std/guiTable.rw"),
        ),
        (
            "guiWindows",
            include_str!("../../../libraries/std/guiWindows.rw"),
        ),
        ("args", include_str!("../../../libraries/std/args.rw")),
        ("bits", include_str!("../../../libraries/std/bits.rw")),
        ("bytes", include_str!("../../../libraries/std/bytes.rw")),
        (
            "collections",
            include_str!("../../../libraries/std/collections.rw"),
        ),
        ("config", include_str!("../../../libraries/std/config.rw")),
        ("json", include_str!("../../../libraries/std/json.rw")),
        ("map", include_str!("../../../libraries/std/map.rw")),
        (
            "numericIndex",
            include_str!("../../../libraries/std/numericIndex.rw"),
        ),
        (
            "numericRange",
            include_str!("../../../libraries/std/numericRange.rw"),
        ),
        ("numeric", include_str!("../../../libraries/std/numeric.rw")),
        ("task", include_str!("../../../libraries/std/task.rw")),
        (
            "numericAsync",
            include_str!("../../../libraries/std/numericAsync.rw"),
        ),
        ("math", include_str!("../../../libraries/std/math.rw")),
        ("number", include_str!("../../../libraries/std/number.rw")),
        ("option", include_str!("../../../libraries/std/option.rw")),
        ("result", include_str!("../../../libraries/std/result.rw")),
        ("text", include_str!("../../../libraries/std/text.rw")),
        ("sort", include_str!("../../../libraries/std/sort.rw")),
        ("search", include_str!("../../../libraries/std/search.rw")),
        (
            "sequence",
            include_str!("../../../libraries/std/sequence.rw"),
        ),
        ("heap", include_str!("../../../libraries/std/heap.rw")),
        ("deque", include_str!("../../../libraries/std/deque.rw")),
        (
            "disjointSet",
            include_str!("../../../libraries/std/disjointSet.rw"),
        ),
        ("integer", include_str!("../../../libraries/std/integer.rw")),
        ("modular", include_str!("../../../libraries/std/modular.rw")),
        ("fenwick", include_str!("../../../libraries/std/fenwick.rw")),
        ("segment", include_str!("../../../libraries/std/segment.rw")),
        (
            "graphLarge",
            include_str!("../../../libraries/std/graphLarge.rw"),
        ),
        (
            "training",
            include_str!("../../../libraries/std/training.rw"),
        ),
        ("graph", include_str!("../../../libraries/std/graph.rw")),
        ("scanner", include_str!("../../../libraries/std/scanner.rw")),
        ("stream", include_str!("../../../libraries/std/stream.rw")),
        ("path", include_str!("../../../libraries/std/path.rw")),
        ("tests", include_str!("../../../libraries/std/tests.rw")),
    ])
}
fn write(root: &Path, path: &str, bytes: impl AsRef<[u8]>) -> Result<()> {
    let full = root.join(path);
    fs::create_dir_all(full.parent().unwrap())?;
    fs::write(full, bytes)?;
    Ok(())
}
fn executable(meta: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        let _ = meta;
        false
    }
}
fn inventory(root: &Path) -> Result<J> {
    fn walk(
        root: &Path,
        dir: &Path,
        out: &mut serde_json::Map<String, J>,
        total: &mut u64,
        depth: usize,
        visited: &mut usize,
    ) -> Result<()> {
        *visited += 1;
        if depth > 64 || *visited > 1024 {
            return Err(invalid("directory budget exceeded"));
        }
        let mut entries = fs::read_dir(dir)?
            .map(|e| e.map(|e| e.path()))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        entries.sort();
        for path in entries {
            let meta = fs::symlink_metadata(&path)?;
            if meta.file_type().is_symlink() {
                return Err(invalid("symlink in distribution"));
            }
            if meta.is_dir() {
                walk(root, &path, out, total, depth + 1, visited)?;
                continue;
            }
            if !meta.is_file() {
                return Err(invalid("unsupported distribution file"));
            }
            let relative = path
                .strip_prefix(root)
                .map_err(|_| invalid("non-relative file"))?
                .to_str()
                .ok_or_else(|| invalid("non-UTF8 path"))?
                .replace('\\', "/");
            if matches!(relative.as_str(), "sdk.json" | "sdk.json.signature") {
                continue;
            }
            *total = total
                .checked_add(meta.len())
                .ok_or_else(|| invalid("byte budget exceeded"))?;
            if *total > MAX_BYTES || out.len() >= MAX_FILES {
                return Err(invalid("distribution budget exceeded"));
            }
            out.insert(relative,json!({"sha256":packages::hash(fs::read(path)?),"bytes":meta.len(),"executable":executable(&meta)}));
        }
        Ok(())
    }
    let mut files = serde_json::Map::new();
    walk(root, root, &mut files, &mut 0, 0, &mut 0)?;
    Ok(J::Object(files))
}
pub(in crate::v2) fn build(output: &Path, key: &Path) -> Result<()> {
    if !matches!(target().as_str(), "x86_64-linux" | "x86_64-windows") {
        return Err(invalid(
            "SDK assembly supports Linux x86_64 and Windows x86_64",
        ));
    }
    // Read/check the caller's seed before creating an owned new directory.
    let seed: [u8; 32] = packages::decode_hex(fs::read_to_string(key)?.trim())?
        .try_into()
        .map_err(|_| invalid("seed must contain 32 bytes"))?;
    let public = packages::encode_hex(
        &ed25519_dalek::SigningKey::from_bytes(&seed)
            .verifying_key()
            .to_bytes(),
    );
    fs::create_dir(output)?;
    let result = (|| {
        let output = fs::canonicalize(output)?;
        let binary = std::env::current_exe()?;
        fs::create_dir(output.join("bin"))?;
        fs::copy(
            &binary,
            output.join(format!("bin/rewind{}", std::env::consts::EXE_SUFFIX)),
        )?;
        fs::copy(
            &binary,
            output.join(format!("bin/rewindc{}", std::env::consts::EXE_SUFFIX)),
        )?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(
                output.join(format!("bin/rewind{}", std::env::consts::EXE_SUFFIX)),
                fs::Permissions::from_mode(0o755),
            )?;
            fs::set_permissions(
                output.join(format!("bin/rewindc{}", std::env::consts::EXE_SUFFIX)),
                fs::Permissions::from_mode(0o755),
            )?;
        }
        for (name, source) in modules() {
            write(&output, &format!("lib/rewind/std/{name}.rw"), source)?;
        }
        write(
            &output,
            "lib/rewind/std/rewind.toml",
            include_str!("../../../libraries/std/rewind.toml"),
        )?;
        write(&output,"lib/rewind/std/rewind.package.json",serde_json::to_vec_pretty(&json!({"name":"std","version":env!("CARGO_PKG_VERSION"),"compiler":env!("CARGO_PKG_VERSION"),"language":env!("CARGO_PKG_VERSION"),"effects":["gui","external","live","clock","network","db","tasks","random","fileRead","fileWrite"],"dependencies":{}})).map_err(|e|invalid(&e.to_string()))?)?;
        let std_root = output.join("lib/rewind/std");
        update_project(&std_root)?;
        // A distribution is only assembled after its embedded standard library
        // is checked and all library contract tests pass.
        cli("test", "", &std_root, false, RunOptions::default())?;
        for name in modules().keys().filter(|n| **n != "tests") {
            let doc = documentation(
                &std_root.join(format!("{name}.rw")).to_string_lossy(),
                &std_root,
            )?;
            write(&output, &format!("share/rewind/doc/std/{name}.md"), doc)?;
        }
        // API snapshots describe a module's own exports, not its imports.
        // Switch the isolated package entry while generating each module snapshot.
        let std_manifest = fs::read_to_string(std_root.join("rewind.toml"))?;
        let mut api_index = serde_json::Map::new();
        for name in modules().keys().filter(|n| **n != "tests") {
            fs::write(
                std_root.join("rewind.toml"),
                std_manifest.replace("tests.rw", &format!("{name}.rw")),
            )?;
            update_project(&std_root)?;
            let relative = format!("std/{name}.api.json");
            v07::api_snapshot(
                &std_root,
                Some(&output.join("share/rewind/doc").join(&relative)),
            )?;
            api_index.insert((*name).into(), J::String(relative));
        }
        fs::write(std_root.join("rewind.toml"), std_manifest)?;
        update_project(&std_root)?;
        write(&output,"share/rewind/doc/std-api.json",serde_json::to_vec_pretty(&json!({"format":1,"compiler":env!("CARGO_PKG_VERSION"),"language":env!("CARGO_PKG_VERSION"),"modules":api_index})).map_err(|e|invalid(&e.to_string()))?)?;
        let cache = std_root.join(".rewind");
        if cache.exists() {
            fs::remove_dir_all(cache)?;
        }
        packages::sign_package(&std_root, key, &std_root.join("rewind.signature"))?;
        write(
            &output,
            "share/rewind/doc/libraries.md",
            include_str!("../../../libraries/README.md")
                .replace("../docs/", "")
                .replace("[v0.9.2仕様](REWIND_v0.9.2.md)", "[SDK guide](language.md)")
                .replace(
                    "[v0.9.3草案](../docs/REWIND_v0.9.3.md)",
                    "[v0.9.3草案](REWIND_v0.9.3.md)",
                ),
        )?;
        write(
            &output,
            "share/rewind/doc/language-reference.md",
            include_str!("../../../docs/language-reference.md")
                .replace("../libraries/README.md", "libraries.md"),
        )?;
        write(
            &output,
            "share/rewind/doc/REWIND_v1.1.md",
            include_str!("../../../docs/REWIND_v1.1.md"),
        )?;
        write(
            &output,
            "share/rewind/doc/REWIND_v1.4.md",
            include_str!("../../../docs/REWIND_v1.4.md"),
        )?;
        write(
            &output,
            "share/rewind/doc/REWIND_v1.5.md",
            include_str!("../../../docs/REWIND_v1.5.md"),
        )?;
        write(
            &output,
            "share/rewind/doc/REWIND_v1.6.md",
            include_str!("../../../docs/REWIND_v1.6.md"),
        )?;
        write(
            &output,
            "share/rewind/doc/REWIND_v1.6.1.md",
            include_str!("../../../docs/REWIND_v1.6.1.md"),
        )?;
        write(
            &output,
            "share/rewind/examples/http/main.rw",
            include_str!("../../../examples/http/main.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/http-stream/main.rw",
            include_str!("../../../examples/http-stream/main.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/http-stream/README.md",
            include_str!("../../../examples/http-stream/README.md")
                .replace("../../docs/REWIND_v1.6.1.md", "../../doc/REWIND_v1.6.1.md"),
        )?;
        write(
            &output,
            "share/rewind/doc/v2-design.md",
            include_str!("../../../docs/v2-design.md"),
        )?;
        write(
            &output,
            "share/rewind/doc/REWIND_v1.8.3.md",
            include_str!("../../../docs/REWIND_v1.8.3.md"),
        )?;
        write(
            &output,
            "share/rewind/doc/REWIND_v1.8.6.md",
            include_str!("../../../docs/REWIND_v1.8.6.md"),
        )?;
        write(
            &output,
            "share/rewind/doc/REWIND_v1.8.7.md",
            include_str!("../../../docs/REWIND_v1.8.7.md"),
        )?;
        write(
            &output,
            "share/rewind/doc/REWIND_v1.8.8.md",
            include_str!("../../../docs/REWIND_v1.8.8.md"),
        )?;
        write(
            &output,
            "share/rewind/examples/tcp/main.rw",
            include_str!("../../../examples/tcp/main.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/tcp/README.md",
            include_str!("../../../examples/tcp/README.md"),
        )?;
        write(
            &output,
            "share/rewind/examples/http-server-auth/main.rw",
            include_str!("../../../examples/http-server-auth/main.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/http-server-auth/README.md",
            include_str!("../../../examples/http-server-auth/README.md"),
        )?;
        write(
            &output,
            "share/rewind/examples/http-server-tls/main.rw",
            include_str!("../../../examples/http-server-tls/main.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/http-server-tls/README.md",
            include_str!("../../../examples/http-server-tls/README.md"),
        )?;
        write(
            &output,
            "share/rewind/examples/http-server/main.rw",
            include_str!("../../../examples/http-server/main.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/http-server/README.md",
            include_str!("../../../examples/http-server/README.md"),
        )?;
        write(
            &output,
            "share/rewind/examples/fft-async/main.rw",
            include_str!("../../../examples/fft-async/main.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/fft-async/README.md",
            include_str!("../../../examples/fft-async/README.md"),
        )?;
        write(
            &output,
            "share/rewind/examples/sparse-async/main.rw",
            include_str!("../../../examples/sparse-async/main.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/sparse-async/README.md",
            include_str!("../../../examples/sparse-async/README.md"),
        )?;
        write(
            &output,
            "share/rewind/examples/vector-async/main.rw",
            include_str!("../../../examples/vector-async/main.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/vector-async/README.md",
            include_str!("../../../examples/vector-async/README.md"),
        )?;
        write(
            &output,
            "share/rewind/examples/gui-data/main.rw",
            include_str!("../../../examples/gui-data/main.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/gui-data/README.md",
            include_str!("../../../examples/gui-data/README.md"),
        )?;
        write(
            &output,
            "share/rewind/examples/solve-async/main.rw",
            include_str!("../../../examples/solve-async/main.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/solve-async/README.md",
            include_str!("../../../examples/solve-async/README.md"),
        )?;
        write(
            &output,
            "share/rewind/examples/http-pool/main.rw",
            include_str!("../../../examples/http-pool/main.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/http-pool/README.md",
            include_str!("../../../examples/http-pool/README.md"),
        )?;
        write(
            &output,
            "share/rewind/examples/text-storage/main.rw",
            include_str!("../../../examples/text-storage/main.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/text-storage/README.md",
            include_str!("../../../examples/text-storage/README.md"),
        )?;
        write(
            &output,
            "share/rewind/examples/qr-async/main.rw",
            include_str!("../../../examples/qr-async/main.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/qr-async/README.md",
            include_str!("../../../examples/qr-async/README.md"),
        )?;
        write(
            &output,
            "share/rewind/examples/eigen-async/main.rw",
            include_str!("../../../examples/eigen-async/main.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/eigen-async/README.md",
            include_str!("../../../examples/eigen-async/README.md"),
        )?;
        write(
            &output,
            "share/rewind/examples/numeric-digests/main.rw",
            include_str!("../../../examples/numeric-digests/main.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/numeric-digests/README.md",
            include_str!("../../../examples/numeric-digests/README.md"),
        )?;
        write(
            &output,
            "share/rewind/examples/service-data/main.rw",
            include_str!("../../../examples/service-data/main.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/service-data/README.md",
            include_str!("../../../examples/service-data/README.md"),
        )?;
        write(
            &output,
            "share/rewind/examples/least-squares-async/main.rw",
            include_str!("../../../examples/least-squares-async/main.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/least-squares-async/README.md",
            include_str!("../../../examples/least-squares-async/README.md"),
        )?;
        write(
            &output,
            "share/rewind/examples/text-admission/main.rw",
            include_str!("../../../examples/text-admission/main.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/text-admission/README.md",
            include_str!("../../../examples/text-admission/README.md"),
        )?;
        write(
            &output,
            "share/rewind/examples/bytes-admission/main.rw",
            include_str!("../../../examples/bytes-admission/main.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/bytes-admission/README.md",
            include_str!("../../../examples/bytes-admission/README.md"),
        )?;
        write(
            &output,
            "share/rewind/examples/container-admission/main.rw",
            include_str!("../../../examples/container-admission/main.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/container-admission/README.md",
            include_str!("../../../examples/container-admission/README.md"),
        )?;
        write(
            &output,
            "share/rewind/examples/numeric-index/main.rw",
            include_str!("../../../examples/numeric-index/main.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/numeric-index/README.md",
            include_str!("../../../examples/numeric-index/README.md"),
        )?;
        write(
            &output,
            "share/rewind/examples/graph-large/main.rw",
            include_str!("../../../examples/graph-large/main.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/nonlinear-training/main.rw",
            include_str!("../../../examples/nonlinear-training/main.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/nonlinear-training/trace-small.rw",
            include_str!("../../../examples/nonlinear-training/trace-small.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/graph-large/README.md",
            include_str!("../../../examples/graph-large/README.md"),
        )?;
        write(
            &output,
            "share/rewind/examples/nonlinear-training/README.md",
            include_str!("../../../examples/nonlinear-training/README.md"),
        )?;
        for (name, doc) in [
            (
                "REWIND_v1.9.49.md",
                include_str!("../../../docs/REWIND_v1.9.49.md"),
            ),
            (
                "REWIND_v1.9.50.md",
                include_str!("../../../docs/REWIND_v1.9.50.md"),
            ),
            (
                "REWIND_v1.9.48.md",
                include_str!("../../../docs/REWIND_v1.9.48.md"),
            ),
            (
                "REWIND_v1.9.47.md",
                include_str!("../../../docs/REWIND_v1.9.47.md"),
            ),
            (
                "REWIND_v1.9.46.md",
                include_str!("../../../docs/REWIND_v1.9.46.md"),
            ),
            (
                "REWIND_v1.9.45.md",
                include_str!("../../../docs/REWIND_v1.9.45.md"),
            ),
            (
                "REWIND_v1.9.44.md",
                include_str!("../../../docs/REWIND_v1.9.44.md"),
            ),
            (
                "REWIND_v1.9.43.md",
                include_str!("../../../docs/REWIND_v1.9.43.md"),
            ),
            (
                "REWIND_v1.9.42.md",
                include_str!("../../../docs/REWIND_v1.9.42.md"),
            ),
            (
                "REWIND_v1.9.41.md",
                include_str!("../../../docs/REWIND_v1.9.41.md"),
            ),
            (
                "REWIND_v1.9.40.md",
                include_str!("../../../docs/REWIND_v1.9.40.md"),
            ),
            (
                "REWIND_v1.9.39.md",
                include_str!("../../../docs/REWIND_v1.9.39.md"),
            ),
            (
                "REWIND_v1.9.38.md",
                include_str!("../../../docs/REWIND_v1.9.38.md"),
            ),
            (
                "REWIND_v1.9.37.md",
                include_str!("../../../docs/REWIND_v1.9.37.md"),
            ),
            (
                "REWIND_v1.9.36.md",
                include_str!("../../../docs/REWIND_v1.9.36.md"),
            ),
            (
                "REWIND_v1.9.35.md",
                include_str!("../../../docs/REWIND_v1.9.35.md"),
            ),
            (
                "REWIND_v1.9.34.md",
                include_str!("../../../docs/REWIND_v1.9.34.md"),
            ),
            (
                "REWIND_v1.9.33.md",
                include_str!("../../../docs/REWIND_v1.9.33.md"),
            ),
            (
                "REWIND_v1.9.32.md",
                include_str!("../../../docs/REWIND_v1.9.32.md"),
            ),
            (
                "REWIND_v1.9.31.md",
                include_str!("../../../docs/REWIND_v1.9.31.md"),
            ),
            (
                "REWIND_v1.9.30.md",
                include_str!("../../../docs/REWIND_v1.9.30.md"),
            ),
            (
                "REWIND_v1.9.29.md",
                include_str!("../../../docs/REWIND_v1.9.29.md"),
            ),
            (
                "REWIND_v1.9.28.md",
                include_str!("../../../docs/REWIND_v1.9.28.md"),
            ),
            (
                "REWIND_v1.9.27.md",
                include_str!("../../../docs/REWIND_v1.9.27.md"),
            ),
            (
                "REWIND_v1.9.26.md",
                include_str!("../../../docs/REWIND_v1.9.26.md"),
            ),
            (
                "REWIND_v1.9.25.md",
                include_str!("../../../docs/REWIND_v1.9.25.md"),
            ),
            (
                "REWIND_v1.9.24.md",
                include_str!("../../../docs/REWIND_v1.9.24.md"),
            ),
            (
                "REWIND_v1.9.23.md",
                include_str!("../../../docs/REWIND_v1.9.23.md"),
            ),
            (
                "REWIND_v1.9.22.md",
                include_str!("../../../docs/REWIND_v1.9.22.md"),
            ),
            (
                "REWIND_v1.9.21.md",
                include_str!("../../../docs/REWIND_v1.9.21.md"),
            ),
            (
                "REWIND_v1.9.20.md",
                include_str!("../../../docs/REWIND_v1.9.20.md"),
            ),
            (
                "REWIND_v1.9.19.md",
                include_str!("../../../docs/REWIND_v1.9.19.md"),
            ),
            (
                "REWIND_v1.9.18.md",
                include_str!("../../../docs/REWIND_v1.9.18.md"),
            ),
            (
                "REWIND_v1.9.17.md",
                include_str!("../../../docs/REWIND_v1.9.17.md"),
            ),
            (
                "REWIND_v1.9.13.md",
                include_str!("../../../docs/REWIND_v1.9.13.md"),
            ),
            (
                "REWIND_v1.9.14.md",
                include_str!("../../../docs/REWIND_v1.9.14.md"),
            ),
            (
                "REWIND_v1.9.16.md",
                include_str!("../../../docs/REWIND_v1.9.16.md"),
            ),
            (
                "REWIND_v1.9.15.md",
                include_str!("../../../docs/REWIND_v1.9.15.md"),
            ),
            (
                "REWIND_v1.9.12.md",
                include_str!("../../../docs/REWIND_v1.9.12.md"),
            ),
            (
                "REWIND_v1.9.11.md",
                include_str!("../../../docs/REWIND_v1.9.11.md"),
            ),
            (
                "REWIND_v1.9.10.md",
                include_str!("../../../docs/REWIND_v1.9.10.md"),
            ),
            (
                "REWIND_v1.9.9.md",
                include_str!("../../../docs/REWIND_v1.9.9.md"),
            ),
            (
                "REWIND_v1.9.8.md",
                include_str!("../../../docs/REWIND_v1.9.8.md"),
            ),
            (
                "REWIND_v1.9.7.md",
                include_str!("../../../docs/REWIND_v1.9.7.md"),
            ),
            (
                "REWIND_v1.9.6.md",
                include_str!("../../../docs/REWIND_v1.9.6.md"),
            ),
            (
                "REWIND_v1.9.5.md",
                include_str!("../../../docs/REWIND_v1.9.5.md"),
            ),
            (
                "REWIND_v1.9.4.md",
                include_str!("../../../docs/REWIND_v1.9.4.md"),
            ),
            (
                "REWIND_v1.9.3.md",
                include_str!("../../../docs/REWIND_v1.9.3.md"),
            ),
            (
                "REWIND_v1.9.2.md",
                include_str!("../../../docs/REWIND_v1.9.2.md"),
            ),
            (
                "REWIND_v1.9.1.md",
                include_str!("../../../docs/REWIND_v1.9.1.md"),
            ),
            (
                "REWIND_v1.9.md",
                include_str!("../../../docs/REWIND_v1.9.md"),
            ),
            (
                "REWIND_v1.8.4.md",
                include_str!("../../../docs/REWIND_v1.8.4.md"),
            ),
            (
                "REWIND_v1.8.5.md",
                include_str!("../../../docs/REWIND_v1.8.5.md"),
            ),
        ] {
            write(&output, &format!("share/rewind/doc/{name}"), doc)?;
        }
        write(
            &output,
            "share/rewind/examples/numeric-index/main.rw",
            include_str!("../../../examples/numeric-index/main.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/numeric-index/README.md",
            include_str!("../../../examples/numeric-index/README.md"),
        )?;
        write(
            &output,
            "share/rewind/examples/graph-large/main.rw",
            include_str!("../../../examples/graph-large/main.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/nonlinear-training/main.rw",
            include_str!("../../../examples/nonlinear-training/main.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/nonlinear-training/trace-small.rw",
            include_str!("../../../examples/nonlinear-training/trace-small.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/graph-large/README.md",
            include_str!("../../../examples/graph-large/README.md"),
        )?;
        write(
            &output,
            "share/rewind/examples/nonlinear-training/README.md",
            include_str!("../../../examples/nonlinear-training/README.md"),
        )?;
        for (name, doc) in [
            (
                "REWIND_v1.9.49.md",
                include_str!("../../../docs/REWIND_v1.9.49.md"),
            ),
            (
                "REWIND_v1.9.50.md",
                include_str!("../../../docs/REWIND_v1.9.50.md"),
            ),
            (
                "REWIND_v1.9.48.md",
                include_str!("../../../docs/REWIND_v1.9.48.md"),
            ),
            (
                "REWIND_v1.9.44.md",
                include_str!("../../../docs/REWIND_v1.9.44.md"),
            ),
            (
                "gui-controls",
                include_str!("../../../examples/gui-controls/README.md"),
            ),
            (
                "json-stream",
                include_str!("../../../examples/json-stream/README.md"),
            ),
            (
                "csv-stream",
                include_str!("../../../examples/csv-stream/README.md"),
            ),
            ("regex", include_str!("../../../examples/regex/README.md")),
            (
                "datetime-sqlite",
                include_str!("../../../examples/datetime-sqlite/README.md"),
            ),
            (
                "datetime-postgres",
                include_str!("../../../examples/datetime-postgres/README.md"),
            ),
            (
                "numeric-memory",
                include_str!("../../../examples/numeric-memory/README.md"),
            ),
            (
                "numeric-async",
                include_str!("../../../examples/numeric-async/README.md"),
            ),
            (
                "advanced-algorithms",
                include_str!("../../../examples/advanced-algorithms/README.md"),
            ),
            (
                "gui-windows",
                include_str!("../../../examples/gui-windows/README.md"),
            ),
            (
                "gui-grapheme",
                include_str!("../../../examples/gui-grapheme/README.md"),
            ),
            (
                "unicode",
                include_str!("../../../examples/unicode/README.md"),
            ),
            (
                "datetime",
                include_str!("../../../examples/datetime/README.md"),
            ),
            (
                "decimal",
                include_str!("../../../examples/decimal/README.md"),
            ),
            (
                "decimal-postgres",
                include_str!("../../../examples/decimal-postgres/README.md"),
            ),
            (
                "decimal-sqlite",
                include_str!("../../../examples/decimal-sqlite/README.md"),
            ),
        ] {
            write(
                &output,
                &format!("share/rewind/examples/{name}/README.md"),
                doc.replace("../../docs/", "../../doc/"),
            )?;
        }
        for (name, source) in [
            (
                "gui-windows",
                include_str!("../../../examples/gui-windows/main.rw"),
            ),
            (
                "gui-controls",
                include_str!("../../../examples/gui-controls/main.rw"),
            ),
            (
                "gui-grapheme",
                include_str!("../../../examples/gui-grapheme/main.rw"),
            ),
            (
                "numeric-memory",
                include_str!("../../../examples/numeric-memory/main.rw"),
            ),
            (
                "numeric-async",
                include_str!("../../../examples/numeric-async/main.rw"),
            ),
            (
                "advanced-algorithms",
                include_str!("../../../examples/advanced-algorithms/main.rw"),
            ),
            ("flow", include_str!("../../../examples/flow/main.rw")),
            ("regex", include_str!("../../../examples/regex/main.rw")),
            (
                "json-stream",
                include_str!("../../../examples/json-stream/main.rw"),
            ),
            (
                "csv-stream",
                include_str!("../../../examples/csv-stream/main.rw"),
            ),
            ("unicode", include_str!("../../../examples/unicode/main.rw")),
            (
                "datetime",
                include_str!("../../../examples/datetime/main.rw"),
            ),
            (
                "datetime-sqlite",
                include_str!("../../../examples/datetime-sqlite/main.rw"),
            ),
            (
                "datetime-postgres",
                include_str!("../../../examples/datetime-postgres/main.rw"),
            ),
            ("decimal", include_str!("../../../examples/decimal/main.rw")),
            (
                "decimal-postgres",
                include_str!("../../../examples/decimal-postgres/main.rw"),
            ),
            (
                "decimal-sqlite",
                include_str!("../../../examples/decimal-sqlite/main.rw"),
            ),
        ] {
            write(
                &output,
                &format!("share/rewind/examples/{name}/main.rw"),
                source,
            )?;
        }
        write(
            &output,
            "share/rewind/examples/gui-controls/events.json",
            include_str!("../../../examples/gui-controls/events.json"),
        )?;
        write(
            &output,
            "share/rewind/examples/gui-windows/events.json",
            include_str!("../../../examples/gui-windows/events.json"),
        )?;
        write(
            &output,
            "share/rewind/doc/REWIND_v1.8.2.md",
            include_str!("../../../docs/REWIND_v1.8.2.md"),
        )?;
        write(
            &output,
            "share/rewind/examples/bigint/main.rw",
            include_str!("../../../examples/bigint/main.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/bigint/README.md",
            &include_str!("../../../examples/bigint/README.md")
                .replace("../../docs/", "../../doc/"),
        )?;
        write(
            &output,
            "share/rewind/doc/REWIND_v1.8.1.md",
            include_str!("../../../docs/REWIND_v1.8.1.md"),
        )?;
        write(
            &output,
            "share/rewind/examples/numeric-analysis/main.rw",
            include_str!("../../../examples/numeric-analysis/main.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/numeric-analysis/README.md",
            &include_str!("../../../examples/numeric-analysis/README.md")
                .replace("../../docs/", "../../doc/"),
        )?;
        write(
            &output,
            "share/rewind/doc/REWIND_v1.8.md",
            include_str!("../../../docs/REWIND_v1.8.md"),
        )?;
        write(
            &output,
            "share/rewind/examples/task-readiness/main.rw",
            include_str!("../../../examples/task-readiness/main.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/gui-live/main.rw",
            include_str!("../../../examples/gui-live/main.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/gui-live/README.md",
            include_str!("../../../examples/gui-live/README.md"),
        )?;
        write(
            &output,
            "share/rewind/examples/gui-live/events.json",
            include_str!("../../../examples/gui-live/events.json"),
        )?;
        write(
            &output,
            "share/rewind/examples/live-external/main.rw",
            include_str!("../../../examples/live-external/main.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/live-external/README.md",
            include_str!("../../../examples/live-external/README.md"),
        )?;
        write(
            &output,
            "share/rewind/examples/task-quota/main.rw",
            include_str!("../../../examples/task-quota/main.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/task-quota/README.md",
            include_str!("../../../examples/task-quota/README.md"),
        )?;
        write(
            &output,
            "share/rewind/examples/task-readiness/README.md",
            include_str!("../../../examples/task-readiness/README.md"),
        )?;
        write(
            &output,
            "share/rewind/examples/gui-async/main.rw",
            include_str!("../../../examples/gui-async/main.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/gui-async/README.md",
            include_str!("../../../examples/gui-async/README.md"),
        )?;
        write(
            &output,
            "share/rewind/examples/model-training/main.rw",
            include_str!("../../../examples/model-training/main.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/model-training/README.md",
            include_str!("../../../examples/model-training/README.md"),
        )?;
        write(
            &output,
            "share/rewind/examples/sparse-fft/main.rw",
            include_str!("../../../examples/sparse-fft/main.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/sparse-fft/README.md",
            include_str!("../../../examples/sparse-fft/README.md"),
        )?;
        write(
            &output,
            "share/rewind/examples/numeric/main.rw",
            include_str!("../../../examples/numeric/main.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/numeric/README.md",
            &include_str!("../../../examples/numeric/README.md")
                .replace("../../docs/", "../../doc/"),
        )?;
        write(
            &output,
            "share/rewind/doc/REWIND_v1.7.1.md",
            include_str!("../../../docs/REWIND_v1.7.1.md"),
        )?;
        write(
            &output,
            "share/rewind/examples/postgres/main.rw",
            include_str!("../../../examples/postgres/main.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/postgres/README.md",
            include_str!("../../../examples/postgres/README.md"),
        )?;
        write(
            &output,
            "share/rewind/doc/REWIND_v1.7.md",
            include_str!("../../../docs/REWIND_v1.7.md"),
        )?;
        write(
            &output,
            "share/rewind/examples/database/main.rw",
            include_str!("../../../examples/database/main.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/database/README.md",
            include_str!("../../../examples/database/README.md"),
        )?;
        write(
            &output,
            "share/rewind/examples/external/main.rw",
            include_str!("../../../examples/external/main.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/external/README.md",
            include_str!("../../../examples/external/README.md"),
        )?;
        write(
            &output,
            "share/rewind/doc/ROADMAP_v2.md",
            include_str!("../../../docs/ROADMAP_v2.md"),
        )?;
        write(
            &output,
            "share/rewind/examples/notes/main.rw",
            include_str!("../../../examples/notes/main.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/notes/events.json",
            include_str!("../../../examples/notes/events.json"),
        )?;
        write(
            &output,
            "share/rewind/examples/notes/README.md",
            include_str!("../../../examples/notes/README.md"),
        )?;
        write(
            &output,
            "share/rewind/doc/REWIND_v1.3.md",
            include_str!("../../../docs/REWIND_v1.3.md"),
        )?;
        write(
            &output,
            "share/rewind/doc/gui.md",
            include_str!("../../../docs/gui.md"),
        )?;
        write(
            &output,
            "share/rewind/examples/gui/main.rw",
            include_str!("../../../examples/gui/main.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/gui/events.json",
            include_str!("../../../examples/gui/events.json"),
        )?;
        write(
            &output,
            "share/rewind/examples/gui/README.md",
            include_str!("../../../examples/gui/README.md")
                .replace("../../docs/gui.md", "../../doc/gui.md"),
        )?;
        write(
            &output,
            "share/rewind/doc/REWIND_v1.2.md",
            include_str!("../../../docs/REWIND_v1.2.md"),
        )?;
        write(
            &output,
            "share/rewind/examples/conditional-revert/main.rw",
            include_str!("../../../examples/v12/main.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/conditional-revert/README.md",
            include_str!("../../../examples/v12/README.md"),
        )?;
        write(
            &output,
            "share/rewind/doc/language.md",
            include_str!("../../../docs/sdk-guide.md"),
        )?;
        write(
            &output,
            "share/rewind/doc/REWIND_v0.9.3.md",
            include_str!("../../../docs/REWIND_v0.9.3.md").replace(
                "[v0.9.2実装状況](v0.9.2-status.md)",
                "[SDK guide](language.md)",
            ),
        )?;
        write(
            &output,
            "share/rewind/doc/v0.9.3-status.md",
            include_str!("../../../docs/v0.9.3-status.md")
                .replace("../libraries/README.md", "libraries.md")
                .replace(
                    "../examples/v093/README.md",
                    "../examples/shortest/README.md",
                ),
        )?;
        write(
            &output,
            "share/rewind/doc/REWIND_v0.9.4.md",
            include_str!("../../../docs/REWIND_v0.9.4.md"),
        )?;
        write(
            &output,
            "share/rewind/doc/v0.9.4-status.md",
            include_str!("../../../docs/v0.9.4-status.md")
                .replace("../libraries/README.md", "libraries.md"),
        )?;
        write(
            &output,
            "share/rewind/doc/REWIND_v0.9.5.md",
            include_str!("../../../docs/REWIND_v0.9.5.md"),
        )?;
        write(
            &output,
            "share/rewind/doc/getting-started.md",
            include_str!("../../../docs/getting-started.md")
                .replace("../libraries/README.md", "libraries.md"),
        )?;
        write(
            &output,
            "share/rewind/doc/getting-started-v0.9.3.md",
            include_str!("../../../docs/getting-started-v0.9.3.md"),
        )?;
        write(
            &output,
            "share/rewind/examples/checkpoint/main.rw",
            include_str!("../../../examples/v094/main.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/checkpoint/README.md",
            include_str!("../../../examples/v094/README.md"),
        )?;
        write(
            &output,
            "share/rewind/doc/v0.9.5-status.md",
            include_str!("../../../docs/v0.9.5-status.md"),
        )?;
        write(
            &output,
            "share/rewind/doc/v0.9.6-status.md",
            include_str!("../../../docs/v0.9.6-status.md"),
        )?;
        write(
            &output,
            "share/rewind/doc/v0.9.7-status.md",
            include_str!("../../../docs/v0.9.7-status.md"),
        )?;
        write(
            &output,
            "share/rewind/doc/v0.9.8-status.md",
            include_str!("../../../docs/v0.9.8-status.md"),
        )?;
        write(
            &output,
            "share/rewind/doc/v0.9.9-status.md",
            include_str!("../../../docs/v0.9.9-status.md"),
        )?;
        write(
            &output,
            "share/rewind/doc/REWIND_v1.0.md",
            include_str!("../../../docs/REWIND_v1.0.md"),
        )?;
        write(
            &output,
            "share/rewind/examples/csv/main.rw",
            include_str!("../../../examples/v099/main.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/csv/README.md",
            include_str!("../../../examples/v099/README.md"),
        )?;
        write(
            &output,
            "share/rewind/examples/sum/main.rw",
            include_str!("../../../examples/v092/main.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/sum/rewind.toml",
            include_str!("../../../examples/v092/rewind.toml")
                .replace("0.9.2", env!("CARGO_PKG_VERSION")),
        )?;
        write(
            &output,
            "share/rewind/examples/app/main.rw",
            include_str!("../../../examples/v091/main.rw").replace("import lib.", "import std."),
        )?;
        write(
            &output,
            "share/rewind/examples/app/rewind.toml",
            include_str!("../../../examples/v091/rewind.toml")
                .replace("0.9.1", env!("CARGO_PKG_VERSION")),
        )?;
        write(
            &output,
            "share/rewind/examples/app/assets/config.json",
            include_bytes!("../../../examples/v091/assets/config.json"),
        )?;
        write(
            &output,
            "share/rewind/examples/shortest/main.rw",
            include_str!("../../../examples/v093/main.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/shortest/rewind.toml",
            include_str!("../../../examples/v093/rewind.toml")
                .replace("0.9.3", env!("CARGO_PKG_VERSION")),
        )?;
        write(
            &output,
            "share/rewind/examples/shortest/README.md",
            include_str!("../../../examples/v093/README.md"),
        )?;
        write(
            &output,
            "licenses/REWIND-MIT.txt",
            include_bytes!("../../../LICENSE"),
        )?;
        write(
            &output,
            "licenses/third-party.md",
            include_bytes!("../../../licenses/third-party-sdk.md"),
        )?;
        write(
            &output,
            "share/rewind/Cargo.lock",
            include_bytes!("../../../Cargo.lock"),
        )?;
        let manifest = json!({"format":1,"kind":"rewind-sdk","compiler":env!("CARGO_PKG_VERSION"),"language":env!("CARGO_PKG_VERSION"),"std_version":env!("CARGO_PKG_VERSION"),"target":target(),"std_sha256":packages::package_hash(&std_root)?,"files":inventory(&output)?});
        write(
            &output,
            "sdk.json",
            serde_json::to_vec_pretty(&manifest).map_err(|e| invalid(&e.to_string()))?,
        )?;
        packages::sign_file(
            &output.join("sdk.json"),
            key,
            &output.join("sdk.json.signature"),
            "sdk",
        )?;
        verify(&output, &public)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(output);
    }
    result
}
pub(in crate::v2) fn verify(sdk: &Path, public: &str) -> Result<()> {
    let sdk = fs::canonicalize(sdk)?;
    for name in ["sdk.json", "sdk.json.signature"] {
        let meta = fs::symlink_metadata(sdk.join(name))?;
        if meta.file_type().is_symlink() || !meta.is_file() || meta.len() > 1024 * 1024 {
            return Err(invalid("invalid manifest/signature path or budget"));
        }
    }
    packages::verify_file(
        &sdk.join("sdk.json"),
        &sdk.join("sdk.json.signature"),
        public,
        "sdk",
    )?;
    let manifest: J = serde_json::from_slice(&fs::read(sdk.join("sdk.json"))?)
        .map_err(|e| invalid(&e.to_string()))?;
    if manifest["format"] != 1
        || manifest["kind"] != "rewind-sdk"
        || manifest["compiler"] != env!("CARGO_PKG_VERSION")
        || manifest["language"] != env!("CARGO_PKG_VERSION")
        || manifest["std_version"] != env!("CARGO_PKG_VERSION")
        || manifest["target"] != target()
        || manifest["files"] != inventory(&sdk)?
    {
        return Err(invalid("version, target or file inventory mismatch"));
    }
    for required in [
        &format!("bin/rewind{}", std::env::consts::EXE_SUFFIX),
        &format!("bin/rewindc{}", std::env::consts::EXE_SUFFIX),
        "lib/rewind/std/rewind.package.json",
        "share/rewind/doc/std-api.json",
        "licenses/REWIND-MIT.txt",
        "licenses/third-party.md",
    ] {
        if !manifest["files"][required].is_object() {
            return Err(invalid("required distribution file missing"));
        }
    }
    let std_root = sdk.join("lib/rewind/std");
    if manifest["std_sha256"] != packages::package_hash(&std_root)? {
        return Err(invalid("std package digest mismatch"));
    }
    let public_bytes: [u8; 32] = packages::decode_hex(public)?
        .try_into()
        .map_err(|_| invalid("public key must have 32 bytes"))?;
    let key = ed25519_dalek::VerifyingKey::from_bytes(&public_bytes)
        .map_err(|_| invalid("invalid public key"))?;
    let sig = ed25519_dalek::Signature::from_slice(&packages::decode_hex(
        fs::read_to_string(std_root.join("rewind.signature"))?.trim(),
    )?)
    .map_err(|_| invalid("invalid std signature"))?;
    key.verify_strict(
        packages::signature_message(&packages::package_hash(&std_root)?).as_bytes(),
        &sig,
    )
    .map_err(|_| invalid("std signature mismatch"))?;
    Ok(())
}
fn add_entry(text: &str, section: &str, key: &str, value: &str) -> Result<String> {
    let mut lines = text.lines().map(str::to_string).collect::<Vec<_>>();
    let header = format!("[{section}]");
    let mut in_section = false;
    let mut start = None;
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.split('#').next().unwrap_or("").trim();
        if trimmed.starts_with('[') {
            in_section = trimmed == header;
            if in_section {
                start = Some(i + 1);
            }
        } else if in_section
            && trimmed
                .split_once('=')
                .is_some_and(|(k, _)| k.trim() == key)
        {
            return Err(invalid(
                "namespace/signer entry already exists; explicit upgrade required",
            ));
        }
    }
    let entry = format!("{key} = \"{value}\"");
    if let Some(i) = start {
        lines.insert(i, entry);
    } else {
        lines.push(header);
        lines.push(entry);
    }
    Ok(lines.join("\n") + "\n")
}
fn copy_tree(source: &Path, out: &Path) -> Result<()> {
    for entry in fs::read_dir(source)? {
        let path = entry?.path();
        let meta = fs::symlink_metadata(&path)?;
        if meta.file_type().is_symlink() {
            return Err(invalid("std symlink"));
        }
        let target = out.join(path.file_name().unwrap());
        if meta.is_dir() {
            fs::create_dir(&target)?;
            copy_tree(&path, &target)?;
        } else if meta.is_file() {
            fs::copy(path, target)?;
        } else {
            return Err(invalid("invalid std file"));
        }
    }
    Ok(())
}
pub(in crate::v2) fn install(root: &Path, sdk: &Path, public: &str) -> Result<()> {
    verify(sdk, public)?;
    let sdk = fs::canonicalize(sdk)?;
    let root = fs::canonicalize(root)?;
    let config =
        project::ProjectConfig::load(&root)?.ok_or_else(|| invalid("project manifest required"))?;
    if config.language != env!("CARGO_PKG_VERSION") || config.production {
        return Err(invalid(
            "install requires a development project matching this SDK language",
        ));
    }
    if config.imports.contains_key("std")
        || fs::symlink_metadata(config.source_root.join("std")).is_ok()
        || fs::symlink_metadata(config.source_root.join("std.rw")).is_ok()
    {
        return Err(invalid("std namespace collision"));
    }
    let original = fs::read_to_string(root.join("rewind.toml"))?;
    if original
        .lines()
        .any(|l| l.split_once('=').is_some_and(|(k, _)| k.trim() == "std"))
    {
        return Err(invalid("std namespace already declared"));
    }
    let digest = packages::package_hash(&sdk.join("lib/rewind/std"))?;
    let relative = format!(
        "vendor/rewind-std-{}-{}",
        env!("CARGO_PKG_VERSION"),
        &digest[..16]
    );
    let vendor = root.join("vendor");
    if fs::symlink_metadata(&vendor).is_ok()
        && (fs::symlink_metadata(&vendor)?.file_type().is_symlink() || !vendor.is_dir())
    {
        return Err(invalid("vendor must be an ordinary directory"));
    }
    let mut manifest = add_entry(&original, "dependencies", "std", &relative)?;
    manifest = add_entry(
        &manifest,
        "dependency_versions",
        "std",
        &format!("={}", env!("CARGO_PKG_VERSION")),
    )?;
    manifest = add_entry(&manifest, "dependency_signers", "std", "rewind-sdk")?;
    manifest = add_entry(&manifest, "trust", "rewind-sdk", public)?;
    let old_lock = fs::read(root.join("rewind.lock")).ok();
    fs::create_dir_all(&vendor)?;
    let dest = root.join(&relative);
    if fs::symlink_metadata(&dest).is_ok() {
        return Err(invalid("std destination already exists"));
    }
    fs::create_dir(&dest)?;
    if let Err(error) = copy_tree(&sdk.join("lib/rewind/std"), &dest) {
        let _ = fs::remove_dir_all(&dest);
        return Err(error);
    }
    let result = (|| {
        fs::write(root.join("rewind.toml"), manifest)?;
        update_project(&root)?;
        Ok(())
    })();
    if result.is_err() {
        fs::write(root.join("rewind.toml"), original)?;
        if let Some(old) = old_lock {
            fs::write(root.join("rewind.lock"), old)?;
        } else if root.join("rewind.lock").exists() {
            fs::remove_file(root.join("rewind.lock"))?;
        }
        let _ = fs::remove_dir_all(&dest);
    }
    result
}
