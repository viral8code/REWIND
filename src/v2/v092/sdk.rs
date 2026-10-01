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
fn modules() -> BTreeMap<&'static str, &'static str> {
    BTreeMap::from([
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
        ("math", include_str!("../../../libraries/std/math.rw")),
        ("number", include_str!("../../../libraries/std/number.rw")),
        ("option", include_str!("../../../libraries/std/option.rw")),
        ("result", include_str!("../../../libraries/std/result.rw")),
        ("text", include_str!("../../../libraries/std/text.rw")),
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
    if target() != "x86_64-linux" {
        return Err(invalid(
            "SDK assembly currently requires the tested x86_64-linux target",
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
        fs::copy(&binary, output.join("bin/rewind"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(output.join("bin/rewind"), fs::Permissions::from_mode(0o755))?;
        }
        for (name, source) in modules() {
            write(&output, &format!("lib/rewind/std/{name}.rw"), source)?;
        }
        write(
            &output,
            "lib/rewind/std/rewind.toml",
            include_str!("../../../libraries/std/rewind.toml"),
        )?;
        write(&output,"lib/rewind/std/rewind.package.json",serde_json::to_vec_pretty(&json!({"name":"std","version":env!("CARGO_PKG_VERSION"),"compiler":env!("CARGO_PKG_VERSION"),"language":"0.9.2","effects":[],"dependencies":{}})).map_err(|e|invalid(&e.to_string()))?)?;
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
        write(&output,"share/rewind/doc/std-api.json",serde_json::to_vec_pretty(&json!({"format":1,"compiler":env!("CARGO_PKG_VERSION"),"language":"0.9.2","modules":api_index})).map_err(|e|invalid(&e.to_string()))?)?;
        let cache = std_root.join(".rewind");
        if cache.exists() {
            fs::remove_dir_all(cache)?;
        }
        packages::sign_package(&std_root, key, &std_root.join("rewind.signature"))?;
        write(
            &output,
            "share/rewind/doc/libraries.md",
            include_str!("../../../libraries/README.md")
                .replace(
                    "[v0.9.2仕様](../docs/REWIND_v0.9.2.md)",
                    "[SDK guide](language.md)",
                )
                .replace(
                    "[v0.9.3草案](../docs/REWIND_v0.9.3.md)",
                    "[v0.9.3草案](REWIND_v0.9.3.md)",
                ),
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
            "share/rewind/examples/sum/main.rw",
            include_str!("../../../examples/v092/main.rw"),
        )?;
        write(
            &output,
            "share/rewind/examples/sum/rewind.toml",
            include_str!("../../../examples/v092/rewind.toml"),
        )?;
        write(
            &output,
            "share/rewind/examples/app/main.rw",
            include_str!("../../../examples/v091/main.rw").replace("import lib.", "import std."),
        )?;
        write(
            &output,
            "share/rewind/examples/app/rewind.toml",
            include_str!("../../../examples/v091/rewind.toml").replace("0.9.1", "0.9.2"),
        )?;
        write(
            &output,
            "share/rewind/examples/app/assets/config.json",
            include_bytes!("../../../examples/v091/assets/config.json"),
        )?;
        write(
            &output,
            "licenses/REWIND-MIT.txt",
            include_bytes!("../../../LICENSE"),
        )?;
        write(
            &output,
            "licenses/third-party.md",
            include_bytes!("../../../licenses/third-party-linux-x86_64.md"),
        )?;
        write(
            &output,
            "share/rewind/Cargo.lock",
            include_bytes!("../../../Cargo.lock"),
        )?;
        let manifest = json!({"format":1,"kind":"rewind-sdk","compiler":env!("CARGO_PKG_VERSION"),"language":"0.9.2","std_version":env!("CARGO_PKG_VERSION"),"target":target(),"std_sha256":packages::package_hash(&std_root)?,"files":inventory(&output)?});
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
        || manifest["language"] != "0.9.2"
        || manifest["std_version"] != env!("CARGO_PKG_VERSION")
        || manifest["target"] != target()
        || manifest["files"] != inventory(&sdk)?
    {
        return Err(invalid("version, target or file inventory mismatch"));
    }
    for required in [
        "bin/rewind",
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
    if config.language != "0.9.2" || config.production {
        return Err(invalid(
            "install requires a development language 0.9.2 project",
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
