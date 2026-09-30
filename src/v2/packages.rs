use super::*;
use sha2::{Digest, Sha256};
pub(super) fn hash(bytes: impl AsRef<[u8]>) -> String {
    encode_hex(&Sha256::digest(bytes))
}
pub(super) fn encode_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
pub(super) fn decode_hex(text: &str) -> Result<Vec<u8>> {
    if text.len() % 2 != 0 || !text.is_ascii() {
        return Err(Error::InvalidOperation("invalid hexadecimal data".into()));
    }
    (0..text.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&text[i..i + 2], 16)
                .map_err(|_| Error::InvalidOperation("invalid hexadecimal data".into()))
        })
        .collect()
}
pub(super) fn signature_message(hash: &str) -> String {
    format!("REWIND-PACKAGE-v1\n{hash}")
}
pub(super) fn package_hash(path: &Path) -> Result<String> {
    fn collect(path: &Path, root: &Path, files: &mut BTreeMap<String, Vec<u8>>) -> Result<()> {
        for entry in fs::read_dir(path)? {
            let path = entry?.path();
            if fs::symlink_metadata(&path)?.file_type().is_symlink() {
                return Err(Error::InvalidPath(path.display().to_string()));
            }
            if path.is_dir() {
                collect(&path, root, files)?;
            } else if path.extension().is_some_and(|e| e == "rw")
                || path.file_name().is_some_and(|n| n == "rewind.package.json")
            {
                files.insert(
                    path.strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                    fs::read(path)?,
                );
            }
        }
        Ok(())
    }
    let root = fs::canonicalize(path)?;
    let mut files = BTreeMap::new();
    collect(&root, &root, &mut files)?;
    let mut hasher = Sha256::new();
    for (name, bytes) in files {
        hasher.update((name.len() as u64).to_le_bytes());
        hasher.update(name.as_bytes());
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    }
    Ok(encode_hex(&hasher.finalize()))
}
pub fn sign_package(path: &Path, key_path: &Path, output: &Path) -> Result<()> {
    use ed25519_dalek::{Signer, SigningKey};
    let key_bytes = decode_hex(fs::read_to_string(key_path)?.trim())?;
    let seed: [u8; 32] = key_bytes
        .try_into()
        .map_err(|_| Error::InvalidOperation("signing seed must have 32 bytes".into()))?;
    let key = SigningKey::from_bytes(&seed);
    let signature = key.sign(signature_message(&package_hash(path)?).as_bytes());
    fs::write(output, encode_hex(&signature.to_bytes()) + "\n")?;
    println!("{}", encode_hex(&key.verifying_key().to_bytes()));
    Ok(())
}
pub fn keygen(path: &Path) -> Result<()> {
    use std::io::Write;
    let mut seed = [0u8; 32];
    getrandom::getrandom(&mut seed)
        .map_err(|e| Error::InvalidOperation(format!("secure random source: {e}")))?;
    let mut options = fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all((encode_hex(&seed) + "\n").as_bytes())?;
    file.sync_all()?;
    println!(
        "{}",
        encode_hex(
            &ed25519_dalek::SigningKey::from_bytes(&seed)
                .verifying_key()
                .to_bytes()
        )
    );
    Ok(())
}
fn file_message(path: &Path, kind: &str) -> Result<String> {
    if fs::metadata(path)?.len() > 128 * 1024 * 1024 {
        return Err(Error::InvalidOperation(
            "signed file exceeds 128 MiB budget".into(),
        ));
    }
    if !matches!(
        kind,
        "artifact" | "trace" | "inspection" | "session" | "release"
    ) {
        return Err(Error::InvalidOperation(
            "signature kind must be artifact, trace, inspection, session or release".into(),
        ));
    }
    Ok(format!(
        "REWIND-{}-v1\n{}",
        kind.to_uppercase(),
        hash(fs::read(path)?)
    ))
}
pub fn sign_file(path: &Path, key_path: &Path, output: &Path, kind: &str) -> Result<()> {
    use ed25519_dalek::Signer;
    let seed: [u8; 32] = decode_hex(fs::read_to_string(key_path)?.trim())?
        .try_into()
        .map_err(|_| Error::InvalidOperation("signing seed must have 32 bytes".into()))?;
    let key = ed25519_dalek::SigningKey::from_bytes(&seed);
    fs::write(
        output,
        encode_hex(&key.sign(file_message(path, kind)?.as_bytes()).to_bytes()) + "\n",
    )?;
    println!("{}", encode_hex(&key.verifying_key().to_bytes()));
    Ok(())
}
pub fn verify_file(path: &Path, signature: &Path, public: &str, kind: &str) -> Result<()> {
    let public: [u8; 32] = decode_hex(public)?
        .try_into()
        .map_err(|_| Error::InvalidOperation("public key must have 32 bytes".into()))?;
    let key = ed25519_dalek::VerifyingKey::from_bytes(&public)
        .map_err(|e| Error::InvalidOperation(e.to_string()))?;
    let signature =
        ed25519_dalek::Signature::from_slice(&decode_hex(fs::read_to_string(signature)?.trim())?)
            .map_err(|e| Error::InvalidOperation(e.to_string()))?;
    key.verify_strict(file_message(path, kind)?.as_bytes(), &signature)
        .map_err(|_| Error::InvalidOperation("file signature verification failed".into()))
}
