use super::*;

pub(super) struct ProjectConfig {
    pub source_root: PathBuf,
    pub entry: PathBuf,
    pub imports: BTreeMap<String, PathBuf>,
    pub language: String,
}

fn quoted(value: &str) -> Result<String> {
    value
        .trim()
        .strip_prefix('"')
        .and_then(|v| v.strip_suffix('"'))
        .map(str::to_string)
        .ok_or_else(|| Error::InvalidOperation(format!("expected quoted manifest value: {value}")))
}
fn inside(root: &Path, relative: &str) -> Result<PathBuf> {
    if relative == "." {
        return Ok(fs::canonicalize(root)?);
    }
    let path = Path::new(relative);
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err(Error::InvalidPath(relative.into()));
    }
    let full = fs::canonicalize(root.join(path))?;
    if !full.starts_with(root) {
        return Err(Error::InvalidPath(relative.into()));
    }
    Ok(full)
}
impl ProjectConfig {
    pub fn load(root: &Path) -> Result<Option<Self>> {
        let root = fs::canonicalize(root)?;
        let manifest = root.join("rewind.toml");
        if !manifest.exists() {
            return Ok(None);
        }
        let source = fs::read_to_string(manifest)?;
        let mut section = "";
        let mut language = None;
        let mut source_root = None;
        let mut entry = None;
        let mut deps = BTreeMap::new();
        for (line, text) in source.lines().enumerate() {
            let text = text.split('#').next().unwrap_or("").trim();
            if text.is_empty() {
                continue;
            }
            if text.starts_with('[') {
                if text != "[dependencies]" {
                    return Err(Error::InvalidOperation(format!(
                        "rewind.toml:{}: unsupported section",
                        line + 1
                    )));
                }
                section = "dependencies";
                continue;
            }
            let (key, value) = text.split_once('=').ok_or_else(|| {
                Error::InvalidOperation(format!("rewind.toml:{}: expected key = value", line + 1))
            })?;
            let key = key.trim();
            let value = quoted(value)?;
            if section == "dependencies" {
                if key.is_empty() || deps.insert(key.to_string(), value).is_some() {
                    return Err(Error::InvalidOperation(format!(
                        "rewind.toml:{}: duplicate dependency",
                        line + 1
                    )));
                }
            } else {
                match key {
                    "language" => language = Some(value),
                    "source_root" => source_root = Some(value),
                    "entry" => entry = Some(value),
                    _ => {
                        return Err(Error::InvalidOperation(format!(
                            "rewind.toml:{}: unknown key {key}",
                            line + 1
                        )))
                    }
                }
            }
        }
        let language = language
            .ok_or_else(|| Error::InvalidOperation("rewind.toml: language is required".into()))?;
        if !matches!(language.as_str(), "0.2" | "0.3") {
            return Err(Error::InvalidOperation(format!(
                "unsupported language version {language}"
            )));
        }
        let source_root = inside(&root, &source_root.unwrap_or_else(|| "src".into()))?;
        if !source_root.is_dir() {
            return Err(Error::InvalidPath(source_root.display().to_string()));
        }
        let entry = entry.unwrap_or_else(|| "main.rw".into());
        let entry = inside(&source_root, &entry)?;
        let mut imports = BTreeMap::new();
        imports.insert(String::new(), source_root.clone());
        for (name, path) in deps {
            if name.contains('/') || name.contains('.') || name.is_empty() {
                return Err(Error::InvalidOperation(format!(
                    "invalid dependency name {name}"
                )));
            }
            let full = inside(&root, &path)?;
            if !full.is_dir() {
                return Err(Error::InvalidPath(path));
            }
            imports.insert(name, full);
        }
        Ok(Some(Self {
            source_root,
            entry,
            imports,
            language,
        }))
    }
    fn digest(path: &Path, root: &Path, hash: &mut u128) -> Result<()> {
        let mut children = fs::read_dir(path)?
            .map(|e| e.map(|e| e.path()))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        children.sort();
        for child in children {
            if fs::symlink_metadata(&child)?.file_type().is_symlink() {
                return Err(Error::InvalidPath(child.display().to_string()));
            }
            let full = fs::canonicalize(&child)?;
            if !full.starts_with(root) {
                return Err(Error::InvalidPath(child.display().to_string()));
            }
            if full.is_dir() {
                Self::digest(&full, root, hash)?;
            } else if full.extension().is_some_and(|e| e == "rw") {
                let rel = full
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                for byte in rel
                    .as_bytes()
                    .iter()
                    .copied()
                    .chain([0])
                    .chain(fs::read(&full)?.into_iter())
                {
                    *hash ^= u128::from(byte);
                    *hash = hash.wrapping_mul(0x0000000001000000000000000000013b);
                }
            }
        }
        Ok(())
    }
    pub fn lock(&self, root: &Path, update: bool) -> Result<()> {
        let mut expected = format!(
            "version = 1\nlanguage = \"{}\"\n\n[dependencies]\n",
            self.language
        );
        for (name, path) in &self.imports {
            if name.is_empty() {
                continue;
            }
            let mut hash = 0x6c62272e07bb014262b821756295c58du128;
            Self::digest(path, &fs::canonicalize(root)?, &mut hash)?;
            expected.push_str(&format!("{name} = \"{hash:032x}\"\n"));
        }
        let path = root.join("rewind.lock");
        if !path.exists() || update {
            fs::write(path, expected)?;
        } else if fs::read_to_string(path)? != expected {
            return Err(Error::InvalidOperation(
                "rewind.lock dependency hash mismatch; run 'rewind lock' to update".into(),
            ));
        }
        Ok(())
    }
}
