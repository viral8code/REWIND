//! Authenticated local caches. Package signatures are verified before loading modules.
//! The cache authenticator is outside the project and is never an artifact signature.
use super::*;
use serde::{de::DeserializeOwned, Serialize};
use sha2::{Digest, Sha256};

pub(in crate::v2) fn stamp() -> String {
    static STAMP: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    STAMP
        .get_or_init(|| {
            packages::hash(concat!(
                env!("CARGO_PKG_VERSION"),
                include_str!("../../../Cargo.toml"),
                include_str!("../../../Cargo.lock"),
                include_str!("../../lib.rs"),
                include_str!("../../replay.rs"),
                include_str!("../../journal.rs"),
                include_str!("../../v2.rs"),
                include_str!("../v05.rs"),
                include_str!("../vm.rs"),
                include_str!("../vm/scheduler.rs"),
                include_str!("../vm/extensions.rs"),
                include_str!("../v05/ownership.rs"),
                include_str!("../v05/capabilities.rs"),
                include_str!("../v05/artifact.rs"),
                include_str!("../v05/tooling.rs"),
                include_str!("../v05/tooling/symbols.rs"),
                include_str!("../v06.rs"),
                include_str!("effects.rs"),
                include_str!("captures.rs"),
                include_str!("language.rs"),
                include_str!("library.rs"),
                include_str!("diagnostics.rs"),
                include_str!("cache.rs"),
                include_str!("../project.rs"),
                include_str!("../packages.rs"),
                include_str!("resolver.rs"),
                include_str!("update.rs"),
                include_str!("debug.rs")
            ))
        })
        .clone()
}
pub(in crate::v2) fn key(value: &impl Serialize) -> Option<String> {
    serde_json::to_vec(value).ok().map(packages::hash)
}
fn auth(key: &[u8], bytes: &[u8]) -> Vec<u8> {
    let mut inner = [0x36u8; 64];
    let mut outer = [0x5cu8; 64];
    for (i, byte) in key.iter().enumerate() {
        inner[i] ^= byte;
        outer[i] ^= byte;
    }
    let mut hash = Sha256::new();
    hash.update(inner);
    hash.update(bytes);
    let mut final_hash = Sha256::new();
    final_hash.update(outer);
    final_hash.update(hash.finalize());
    final_hash.finalize().to_vec()
}
#[cfg(unix)]
fn secret(root: &Path) -> Option<Vec<u8>> {
    use std::io::Write;
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
    // SAFETY: geteuid takes no pointers and only reads the process identity.
    let owner = unsafe { libc::geteuid() };
    let id = packages::hash(root.to_string_lossy().as_bytes());
    let path = std::env::temp_dir().join(format!("rewind-cache-auth-{id}"));
    if !path.exists() {
        fs::DirBuilder::new().mode(0o700).create(&path).ok()?;
    }
    let meta = fs::symlink_metadata(&path).ok()?;
    if !meta.is_dir()
        || meta.file_type().is_symlink()
        || meta.uid() != owner
        || meta.mode() & 0o077 != 0
    {
        return None;
    }
    let path = path.join("key");
    if !path.exists() {
        let mut random = vec![0; 32];
        getrandom::getrandom(&mut random).ok()?;
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
        {
            Ok(mut file) => {
                file.write_all(&random).ok()?;
                file.sync_all().ok()?;
            }
            Err(_) if path.exists() => {}
            Err(_) => return None,
        }
    }
    let meta = fs::symlink_metadata(&path).ok()?;
    if !meta.is_file()
        || meta.file_type().is_symlink()
        || meta.uid() != owner
        || meta.len() != 32
        || meta.mode() & 0o077 != 0
    {
        return None;
    }
    fs::read(path).ok()
}
#[cfg(not(unix))]
fn secret(_: &Path) -> Option<Vec<u8>> {
    None
}
fn path(root: &Path, kind: &str, key: &str) -> Option<(PathBuf, Vec<u8>)> {
    if std::env::var_os("REWIND_NO_CACHE").is_some()
        || key.len() != 64
        || !key.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return None;
    }
    let root = fs::canonicalize(root).ok()?;
    let manifest = fs::read_to_string(root.join("rewind.toml")).ok()?;
    if !manifest.lines().any(|line| {
        let line = line.split('#').next().unwrap_or("");
        line.split_once('=')
            .is_some_and(|(key, value)| key.trim() == "language" && value.trim() == "\"0.6\"")
    }) {
        return None;
    }
    let secret = secret(&root)?;
    let parent = root.join(".rewind");
    let cache = parent.join("cache");
    for dir in [&parent, &cache] {
        if fs::symlink_metadata(dir).is_ok_and(|m| m.file_type().is_symlink() || !m.is_dir()) {
            return None;
        }
    }
    fs::create_dir_all(&cache).ok()?;
    let path = cache.join(format!("v06-{kind}-{key}.json"));
    if fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
        return None;
    }
    Some((path, secret))
}
pub(in crate::v2) fn get<T: DeserializeOwned>(root: &Path, kind: &str, key: &str) -> Option<T> {
    let (path, secret) = path(root, kind, key)?;
    if fs::metadata(&path).ok()?.len() > 16 * 1024 * 1024 {
        return None;
    }
    let envelope: serde_json::Value = serde_json::from_slice(&fs::read(path).ok()?).ok()?;
    if envelope["compiler"] != stamp() || envelope["key"] != key || envelope["kind"] != kind {
        return None;
    }
    let bytes = serde_json::to_vec(&serde_json::json!([
        stamp(),
        kind,
        key,
        envelope["payload"]
    ]))
    .ok()?;
    let signature: Vec<u8> = serde_json::from_value(envelope["auth"].clone()).ok()?;
    if signature != auth(&secret, &bytes) {
        return None;
    }
    serde_json::from_value(envelope["payload"].clone()).ok()
}
pub(in crate::v2) fn put(root: &Path, kind: &str, key: &str, value: &impl Serialize) {
    let Some((path, secret)) = path(root, kind, key) else {
        return;
    };
    let Ok(payload) = serde_json::to_value(value) else {
        return;
    };
    let Ok(bytes) = serde_json::to_vec(&serde_json::json!([stamp(), kind, key, payload])) else {
        return;
    };
    if bytes.len() > 8 * 1024 * 1024 {
        return;
    }
    if fs::read_dir(path.parent().unwrap()).map_or(true, |entries| entries.count() >= 512)
        && !path.exists()
    {
        return;
    }
    let value = serde_json::json!({"compiler":stamp(),"key":key,"kind":kind,"auth":auth(&secret,&bytes),"payload":payload});
    let Ok(bytes) = serde_json::to_vec(&value) else {
        return;
    };
    let mut nonce = [0u8; 8];
    if getrandom::getrandom(&mut nonce).is_err() {
        return;
    }
    let temp = path.with_extension(format!("{:x}.tmp", u64::from_le_bytes(nonce)));
    if let Ok(mut file) = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temp)
    {
        use std::io::Write;
        if file.write_all(&bytes).and_then(|_| file.sync_all()).is_ok() {
            let _ = fs::rename(&temp, &path);
        }
        let _ = fs::remove_file(temp);
    }
}
