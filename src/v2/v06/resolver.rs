use super::*;
use serde_json::Value as Json;

fn inside(root: &Path, text: &str) -> Result<PathBuf> {
    let path = Path::new(text.strip_prefix("file:").unwrap_or(text));
    if path.is_absolute()
        || path
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err(Error::InvalidPath(text.into()));
    }
    let path = fs::canonicalize(root.join(path))?;
    if !path.starts_with(root) || !path.is_dir() {
        return Err(Error::InvalidPath(text.into()));
    }
    Ok(path)
}
struct Solver<'a> {
    root: &'a Path,
    sources: BTreeMap<String, String>,
    requirements: &'a BTreeMap<String, String>,
    versions: &'a BTreeMap<String, String>,
    signers: &'a BTreeMap<String, String>,
    trust: &'a BTreeMap<String, String>,
    revoked: &'a BTreeSet<String>,
    locked: Json,
    latest: bool,
    attempts: usize,
}
impl Solver<'_> {
    fn requirements(
        &self,
        selected: &BTreeMap<String, PathBuf>,
    ) -> Result<BTreeMap<String, String>> {
        let mut constraints = self.requirements.clone();
        for path in selected.values() {
            let metadata: Json =
                serde_json::from_slice(&fs::read(path.join("rewind.package.json"))?)
                    .map_err(|e| Error::InvalidOperation(e.to_string()))?;
            if let Some(deps) = metadata.get("dependencies") {
                for (n, r) in deps.as_object().ok_or_else(|| {
                    Error::InvalidOperation("package dependencies must be an object".into())
                })? {
                    let r = r.as_str().ok_or_else(|| {
                        Error::InvalidOperation("transitive requirement must be a string".into())
                    })?;
                    semver::VersionReq::parse(r)
                        .map_err(|e| Error::InvalidOperation(e.to_string()))?;
                    if !constraints.contains_key(n) {
                        if let Some(explicit) = self.versions.get(n) {
                            constraints.insert(n.clone(), explicit.clone());
                        }
                    }
                    constraints
                        .entry(n.clone())
                        .and_modify(|v| {
                            v.push_str(", ");
                            v.push_str(r);
                        })
                        .or_insert_with(|| r.into());
                }
            }
        }
        Ok(constraints)
    }
    fn version(&self, n: &str, path: &Path, wanted: &str) -> Result<semver::Version> {
        use ed25519_dalek::{Signature, VerifyingKey};
        let metadata: Json = serde_json::from_slice(&fs::read(path.join("rewind.package.json"))?)
            .map_err(|e| Error::InvalidOperation(e.to_string()))?;
        if metadata["name"] != n {
            return Err(Error::InvalidOperation("metadata name mismatch".into()));
        }
        let version = semver::Version::parse(metadata["version"].as_str().unwrap_or(""))
            .map_err(|e| Error::InvalidOperation(e.to_string()))?;
        let requirement = semver::VersionReq::parse(wanted)
            .map_err(|e| Error::InvalidOperation(e.to_string()))?;
        if !requirement.matches(&version) {
            return Err(Error::InvalidOperation(format!(
                "{version} does not satisfy {wanted}"
            )));
        }
        let signer = self
            .signers
            .get(n)
            .ok_or_else(|| Error::InvalidOperation(format!("package {n}: missing signer")))?;
        if self.revoked.contains(signer) {
            return Err(Error::InvalidOperation(format!(
                "revoked package signer {signer}"
            )));
        }
        let public = self
            .trust
            .get(signer)
            .ok_or_else(|| Error::InvalidOperation(format!("untrusted package signer {signer}")))?;
        let key: [u8; 32] = packages::decode_hex(public)?
            .try_into()
            .map_err(|_| Error::InvalidOperation("Ed25519 public key must have 32 bytes".into()))?;
        let key =
            VerifyingKey::from_bytes(&key).map_err(|e| Error::InvalidOperation(e.to_string()))?;
        let signature = Signature::from_slice(&packages::decode_hex(
            fs::read_to_string(path.join("rewind.signature"))?.trim(),
        )?)
        .map_err(|e| Error::InvalidOperation(e.to_string()))?;
        key.verify_strict(
            packages::signature_message(&packages::package_hash(path)?).as_bytes(),
            &signature,
        )
        .map_err(|_| Error::InvalidOperation("signature verification failed".into()))?;
        Ok(version)
    }
    fn solve(
        &mut self,
        selected: BTreeMap<String, PathBuf>,
        depth: usize,
    ) -> Result<Option<(BTreeMap<String, PathBuf>, BTreeMap<String, String>)>> {
        self.attempts += 1;
        if self.attempts > 4096 || depth > 128 {
            return Err(Error::InvalidOperation(
                "DependencyResolutionBudgetExceeded".into(),
            ));
        }
        let constraints = match self.requirements(&selected) {
            Ok(c) => c,
            Err(_) => return Ok(None),
        };
        for (n, path) in &selected {
            if self
                .version(
                    n,
                    path,
                    constraints.get(n).map(String::as_str).unwrap_or("*"),
                )
                .is_err()
            {
                return Ok(None);
            }
        }
        let needed = constraints
            .keys()
            .chain(
                self.sources
                    .keys()
                    .filter(|n| self.requirements.contains_key(*n)),
            )
            .filter(|n| !selected.contains_key(*n))
            .cloned()
            .collect::<BTreeSet<_>>();
        let Some(name) = needed.first() else {
            return Ok(Some((selected, constraints)));
        };
        let Some(source) = self.sources.get(name) else {
            return Ok(None);
        };
        let wanted = constraints.get(name).ok_or_else(|| {
            Error::InvalidOperation(format!("package {name}: missing version requirement"))
        })?;
        let mut candidates = Vec::new();
        for candidate in source.split('|') {
            if let Ok(path) = inside(self.root, candidate.trim()) {
                if let Ok(version) = self.version(name, &path, wanted) {
                    candidates.push((version, path));
                }
            }
        }
        candidates.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        if !self.latest {
            if let Some(locked) = self.locked["dependencies"][name]["source"].as_str() {
                let locked = self.root.join(locked);
                candidates.sort_by_key(|(_, p)| p != &locked);
            }
        }
        for (_, path) in candidates {
            let mut next = selected.clone();
            next.insert(name.clone(), path);
            if let Some(solution) = self.solve(next, depth + 1)? {
                return Ok(Some(solution));
            }
        }
        Ok(None)
    }
}
pub(in crate::v2) fn resolve(
    root: &Path,
    deps: &BTreeMap<String, String>,
    registry: &BTreeMap<String, String>,
    requirements: &BTreeMap<String, String>,
    signers: &BTreeMap<String, String>,
    trust: &BTreeMap<String, String>,
    revoked: &BTreeSet<String>,
    latest: bool,
) -> Result<(BTreeMap<String, PathBuf>, BTreeMap<String, String>)> {
    let mut sources = registry.clone();
    sources.extend(deps.clone());
    let mut initial = BTreeMap::new();
    for n in deps.keys() {
        if n.is_empty() || n.contains('/') || n.contains('.') {
            return Err(Error::InvalidOperation(format!(
                "invalid dependency name {n}"
            )));
        }
        initial.insert(
            n.clone(),
            requirements.get(n).cloned().ok_or_else(|| {
                Error::InvalidOperation(format!("package {n}: missing version requirement"))
            })?,
        );
    }
    let locked = fs::read(root.join("rewind.lock"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(Json::Null);
    let mut solver = Solver {
        root,
        sources,
        requirements: &initial,
        versions: requirements,
        signers,
        trust,
        revoked,
        locked,
        latest,
        attempts: 0,
    };
    solver.solve(BTreeMap::new(), 0)?.ok_or_else(|| Error::InvalidOperation("DependencyResolutionFailed: no trusted local candidate graph satisfies all version constraints".into()))
}
