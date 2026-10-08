use super::*;

pub(super) struct ProjectConfig {
    pub source_root: PathBuf,
    pub entry: PathBuf,
    pub imports: BTreeMap<String, PathBuf>,
    lock_imports: BTreeMap<String, PathBuf>,
    pub language: String,
    pub effects: BTreeSet<String>,
    pub(super) production: bool,
    pub(super) assets: BTreeMap<String, String>,
    blocked_sources: Vec<PathBuf>,
    versions: BTreeMap<String, String>,
    signers: BTreeMap<String, String>,
    trust: BTreeMap<String, String>,
    revoked: BTreeSet<String>,
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
struct Manifest {
    language: String,
    source_root: Option<String>,
    entry: Option<String>,
    deps: BTreeMap<String, String>,
    assets: BTreeMap<String, String>,
    dev_deps: BTreeMap<String, String>,
    effects: BTreeSet<String>,
    versions: BTreeMap<String, String>,
    signers: BTreeMap<String, String>,
    trust: BTreeMap<String, String>,
    registry: BTreeMap<String, String>,
    revoked: BTreeSet<String>,
    production: bool,
}
impl Manifest {
    fn load(root: &Path) -> Result<Option<Self>> {
        let manifest = root.join("rewind.toml");
        if !manifest.exists() {
            return Ok(None);
        }
        if fs::metadata(&manifest)?.len() > 1024 * 1024 {
            return Err(Error::InvalidOperation("manifest exceeds 1 MiB".into()));
        }
        let source = fs::read_to_string(manifest)?;
        let mut section = "";
        let mut seen = BTreeSet::new();
        let mut language = None;
        let mut dependency_mode = String::new();
        let mut source_root = None;
        let mut entry = None;
        let mut deps = BTreeMap::new();
        let mut assets = BTreeMap::new();
        let mut dev_deps = BTreeMap::new();
        let mut effects = BTreeSet::new();
        let mut versions = BTreeMap::new();
        let mut signers = BTreeMap::new();
        let mut trust = BTreeMap::new();
        let mut registry = BTreeMap::new();
        let mut revoked = BTreeSet::new();
        for (line, text) in source.lines().enumerate() {
            let text = text.split('#').next().unwrap_or("").trim();
            if text.is_empty() {
                continue;
            }
            if text.starts_with('[') {
                if !matches!(
                    text,
                    "[assets]"
                        | "[dependencies]"
                        | "[dev_dependencies]"
                        | "[dependency_versions]"
                        | "[dependency_signers]"
                        | "[trust]"
                        | "[registry]"
                ) {
                    return Err(Error::InvalidOperation(format!(
                        "rewind.toml:{}: unsupported section",
                        line + 1
                    )));
                }
                section = text.trim_matches(['[', ']']);
                continue;
            }
            let (key, value) = text.split_once('=').ok_or_else(|| {
                Error::InvalidOperation(format!("rewind.toml:{}: expected key = value", line + 1))
            })?;
            let key = key.trim();
            let value = quoted(value)?;
            if !seen.insert((section.to_string(), key.to_string())) {
                return Err(Error::InvalidOperation(format!(
                    "rewind.toml:{}: duplicate key {key}",
                    line + 1
                )));
            }
            if !section.is_empty() {
                let entries = match section {
                    "assets" => &mut assets,
                    "dependencies" => &mut deps,
                    "dev_dependencies" => &mut dev_deps,
                    "dependency_versions" => &mut versions,
                    "dependency_signers" => &mut signers,
                    "registry" => &mut registry,
                    _ => &mut trust,
                };
                if key.is_empty() || entries.insert(key.to_string(), value).is_some() {
                    return Err(Error::InvalidOperation(format!(
                        "rewind.toml:{}: duplicate dependency",
                        line + 1
                    )));
                }
            } else {
                match key {
                    "language" => language = Some(value),
                    "dependency_mode" => dependency_mode = value,
                    "source_root" => source_root = Some(value),
                    "entry" => entry = Some(value),
                    "revoked" => {
                        revoked = value
                            .split(',')
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                            .map(str::to_string)
                            .collect()
                    }
                    "effects" => {
                        effects = value
                            .split(',')
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                            .map(str::to_string)
                            .collect()
                    }
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
        if semver::Version::parse(&language).is_ok_and(|v| {
            v > semver::Version::parse(env!("CARGO_PKG_VERSION")).expect("compiler version")
        }) {
            return Err(Error::InvalidOperation(format!(
                "language {language} requires a newer compiler"
            )));
        }
        if !matches!(
            language.as_str(),
            "0.2"
                | "0.3"
                | "0.4"
                | "0.5"
                | "0.6"
                | "0.7"
                | "0.8"
                | "0.9"
                | "0.9.1"
                | "0.9.2"
                | "0.9.3"
                | "0.9.4"
                | "0.9.5"
                | "0.9.6"
                | "0.9.7"
                | "0.9.8"
                | "0.9.9"
                | "1.0.0"
                | "1.1.0"
                | "1.2.0"
                | "1.3.0"
                | "1.4.0"
                | "1.5.0"
                | "1.6.0"
                | "1.6.1"
                | "1.7.0"
                | "1.7.1"
                | "1.8.0"
                | "1.8.1"
                | "1.8.2"
                | "1.8.3"
                | "1.8.4"
                | "1.8.5"
                | "1.8.6"
                | "1.8.7"
                | "1.8.8"
                | "1.9.0"
                | "1.9.1"
                | "1.9.2"
                | "1.9.3"
                | "1.9.4"
                | "1.9.5"
                | "1.9.6"
                | "1.9.7"
                | "1.9.8"
                | "1.9.9"
                | "1.9.10"
                | "1.9.11"
                | "1.9.12"
                | "1.9.13"
                | "1.9.14"
                | "1.9.15"
                | "1.9.16"
                | "1.9.17"
                | "1.9.18"
                | "1.9.19"
                | "1.9.20"
                | "1.9.21"
                | "1.9.22"
                | "1.9.23"
                | "1.9.24"
                | "1.9.25"
                | "1.9.26"
                | "1.9.27"
                | "1.9.28"
                | "1.9.29"
                | "1.9.30"
                | "1.9.31"
                | "1.9.32"
                | "1.9.33"
                | "1.9.34"
                | "1.9.35"
                | "1.9.36"
                | "1.9.37"
                | "1.9.38"
                | "1.9.39"
                | "1.9.40"
                | "1.9.41"
                | "1.9.42"
                | "1.9.43"
                | "1.9.44"
                | "1.9.45"
                | "1.9.46"
                | "1.9.47"
                | "1.9.48"
                | "2.0.0"
        ) {
            return Err(Error::InvalidOperation(format!(
                "unsupported language version {language}"
            )));
        }
        if !dev_deps.is_empty()
            && !matches!(
                language.as_str(),
                "0.8"
                    | "0.9"
                    | "0.9.1"
                    | "0.9.2"
                    | "0.9.3"
                    | "0.9.4"
                    | "0.9.5"
                    | "0.9.6"
                    | "0.9.7"
                    | "0.9.8"
                    | "0.9.9"
                    | "1.0.0"
                    | "1.1.0"
                    | "1.2.0"
                    | "1.3.0"
                    | "1.4.0"
                    | "1.5.0"
                    | "1.6.0"
                    | "1.6.1"
                    | "1.7.0"
                    | "1.7.1"
                    | "1.8.0"
                    | "1.8.1"
                    | "1.8.2"
                    | "1.8.3"
                    | "1.8.4"
                    | "1.8.5"
                    | "1.8.6"
                    | "1.8.7"
                    | "1.8.8"
                    | "1.9.0"
                    | "1.9.1"
                    | "1.9.2"
                    | "1.9.3"
                    | "1.9.4"
                    | "1.9.5"
                    | "1.9.6"
                    | "1.9.7"
                    | "1.9.8"
                    | "1.9.9"
                    | "1.9.10"
                    | "1.9.11"
                    | "1.9.12"
                    | "1.9.13"
                    | "1.9.14"
                    | "1.9.15"
                    | "1.9.16"
                    | "1.9.17"
                    | "1.9.18"
                    | "1.9.19"
                    | "1.9.20"
                    | "1.9.21"
                    | "1.9.22"
                    | "1.9.23"
                    | "1.9.24"
                    | "1.9.25"
                    | "1.9.26"
                    | "1.9.27"
                    | "1.9.28"
                    | "1.9.29"
                    | "1.9.30"
                    | "1.9.31"
                    | "1.9.32"
                    | "1.9.33"
                    | "1.9.34"
                    | "1.9.35"
                    | "1.9.36"
                    | "1.9.37"
                    | "1.9.38"
                    | "1.9.39"
                    | "1.9.40"
                    | "1.9.41"
                    | "1.9.42"
                    | "1.9.43"
                    | "1.9.44"
                    | "1.9.45"
                    | "1.9.46"
                    | "1.9.47"
                    | "1.9.48"
                    | "2.0.0"
            )
        {
            return Err(Error::InvalidOperation(
                "dev_dependencies requires language 0.8".into(),
            ));
        }
        let production = dependency_mode == "production";
        if !dependency_mode.is_empty()
            && (!matches!(
                language.as_str(),
                "0.9"
                    | "0.9.1"
                    | "0.9.2"
                    | "0.9.3"
                    | "0.9.4"
                    | "0.9.5"
                    | "0.9.6"
                    | "0.9.7"
                    | "0.9.8"
                    | "0.9.9"
                    | "1.0.0"
                    | "1.1.0"
                    | "1.2.0"
                    | "1.3.0"
                    | "1.4.0"
                    | "1.5.0"
                    | "1.6.0"
                    | "1.6.1"
                    | "1.7.0"
                    | "1.7.1"
                    | "1.8.0"
                    | "1.8.1"
                    | "1.8.2"
                    | "1.8.3"
                    | "1.8.4"
                    | "1.8.5"
                    | "1.8.6"
                    | "1.8.7"
                    | "1.8.8"
                    | "1.9.0"
                    | "1.9.1"
                    | "1.9.2"
                    | "1.9.3"
                    | "1.9.4"
                    | "1.9.5"
                    | "1.9.6"
                    | "1.9.7"
                    | "1.9.8"
                    | "1.9.9"
                    | "1.9.10"
                    | "1.9.11"
                    | "1.9.12"
                    | "1.9.13"
                    | "1.9.14"
                    | "1.9.15"
                    | "1.9.16"
                    | "1.9.17"
                    | "1.9.18"
                    | "1.9.19"
                    | "1.9.20"
                    | "1.9.21"
                    | "1.9.22"
                    | "1.9.23"
                    | "1.9.24"
                    | "1.9.25"
                    | "1.9.26"
                    | "1.9.27"
                    | "1.9.28"
                    | "1.9.29"
                    | "1.9.30"
                    | "1.9.31"
                    | "1.9.32"
                    | "1.9.33"
                    | "1.9.34"
                    | "1.9.35"
                    | "1.9.36"
                    | "1.9.37"
                    | "1.9.38"
                    | "1.9.39"
                    | "1.9.40"
                    | "1.9.41"
                    | "1.9.42"
                    | "1.9.43"
                    | "1.9.44"
                    | "1.9.45"
                    | "1.9.46"
                    | "1.9.47"
                    | "1.9.48"
                    | "2.0.0"
            ) || !matches!(dependency_mode.as_str(), "production" | "development"))
        {
            return Err(Error::InvalidOperation(
                "unsupported dependency_mode".into(),
            ));
        }
        Ok(Some(Self {
            language,
            source_root,
            entry,
            deps,
            assets,
            dev_deps,
            effects,
            versions,
            signers,
            trust,
            registry,
            revoked,
            production,
        }))
    }
}
impl ProjectConfig {
    pub(super) fn validate_module_effects(&self, program: &Program) -> Result<()> {
        if program.included_modules.iter().any(|p| {
            self.blocked_sources
                .iter()
                .any(|blocked| p.starts_with(blocked))
        }) {
            return Err(Error::InvalidOperation(
                "development-only module requires a verified development graph".into(),
            ));
        }

        if matches!(
            self.language.as_str(),
            "0.8"
                | "0.9"
                | "0.9.1"
                | "0.9.2"
                | "0.9.3"
                | "0.9.4"
                | "0.9.5"
                | "0.9.6"
                | "0.9.7"
                | "0.9.8"
                | "0.9.9"
                | "1.0.0"
                | "1.1.0"
                | "1.2.0"
                | "1.3.0"
                | "1.4.0"
                | "1.5.0"
                | "1.6.0"
                | "1.6.1"
                | "1.7.0"
                | "1.7.1"
                | "1.8.0"
                | "1.8.1"
                | "1.8.2"
                | "1.8.3"
                | "1.8.4"
                | "1.8.5"
                | "1.8.6"
                | "1.8.7"
                | "1.8.8"
                | "1.9.0"
                | "1.9.1"
                | "1.9.2"
                | "1.9.3"
                | "1.9.4"
                | "1.9.5"
                | "1.9.6"
                | "1.9.7"
                | "1.9.8"
                | "1.9.9"
                | "1.9.10"
                | "1.9.11"
                | "1.9.12"
                | "1.9.13"
                | "1.9.14"
                | "1.9.15"
                | "1.9.16"
                | "1.9.17"
                | "1.9.18"
                | "1.9.19"
                | "1.9.20"
                | "1.9.21"
                | "1.9.22"
                | "1.9.23"
                | "1.9.24"
                | "1.9.25"
                | "1.9.26"
                | "1.9.27"
                | "1.9.28"
                | "1.9.29"
                | "1.9.30"
                | "1.9.31"
                | "1.9.32"
                | "1.9.33"
                | "1.9.34"
                | "1.9.35"
                | "1.9.36"
                | "1.9.37"
                | "1.9.38"
                | "1.9.39"
                | "1.9.40"
                | "1.9.41"
                | "1.9.42"
                | "1.9.43"
                | "1.9.44"
                | "1.9.45"
                | "1.9.46"
                | "1.9.47"
                | "1.9.48"
                | "2.0.0"
        ) {
            for (name, path) in &self.lock_imports {
                if !name.is_empty()
                    && !self.imports.contains_key(name)
                    && program
                        .included_modules
                        .iter()
                        .any(|module| module.starts_with(path))
                {
                    return Err(Error::InvalidOperation(format!(
                        "development-only module {name} requires test/doctest"
                    )));
                }
            }
        }

        for (name, path) in &self.imports {
            if name.is_empty() {
                continue;
            }
            let metadata: serde_json::Value =
                serde_json::from_slice(&fs::read(path.join("rewind.package.json"))?)
                    .map_err(|e| Error::InvalidOperation(e.to_string()))?;
            let allowed = metadata["effects"]
                .as_array()
                .ok_or_else(|| Error::InvalidOperation("missing package effects".into()))?
                .iter()
                .filter_map(|v| v.as_str())
                .map(str::to_string)
                .collect::<BTreeSet<_>>();
            for (module, effects) in &program.module_effects {
                if module.starts_with(path) {
                    if let Some(e) = effects.difference(&allowed).next() {
                        return Err(Error::InvalidOperation(format!(
                            "package {name} -> module {}: undeclared effect {e}",
                            module.display()
                        )));
                    }
                }
            }
        }
        Ok(())
    }
    pub(super) fn artifact(root: &Path, entry: PathBuf, effects: BTreeSet<String>) -> Self {
        Self {
            source_root: root.into(),
            entry,
            imports: BTreeMap::new(),
            lock_imports: BTreeMap::new(),
            language: "0.5".into(),
            production: false,
            assets: BTreeMap::new(),
            blocked_sources: Vec::new(),
            effects,
            versions: BTreeMap::new(),
            signers: BTreeMap::new(),
            trust: BTreeMap::new(),
            revoked: BTreeSet::new(),
        }
    }
    pub fn load(root: &Path) -> Result<Option<Self>> {
        Self::load_mode(root, false, false)
    }
    pub(super) fn load_for_update(root: &Path) -> Result<Option<Self>> {
        Self::load_mode(root, true, true)
    }
    pub(super) fn load_for_test(root: &Path) -> Result<Option<Self>> {
        Self::load_mode(root, false, true)
    }
    fn load_mode(root: &Path, latest: bool, include_dev: bool) -> Result<Option<Self>> {
        let root = fs::canonicalize(root)?;
        let Some(Manifest {
            language,
            source_root,
            entry,
            mut deps,
            assets,
            dev_deps,
            effects,
            mut versions,
            signers,
            trust,
            mut registry,
            revoked,
            production,
        }) = Manifest::load(&root)?
        else {
            return Ok(None);
        };
        if production && (include_dev || latest) {
            return Err(Error::InvalidOperation("production mode has no verified development graph; use development mode for test/update".into()));
        }
        let mut blocked_sources = Vec::new();
        let runtime_roots = deps.keys().cloned().collect::<BTreeSet<_>>();
        for (name, source) in dev_deps {
            if production {
                for candidate in source.split('|') {
                    let relative = candidate
                        .trim()
                        .strip_prefix("file:")
                        .unwrap_or(candidate.trim());
                    let path = Path::new(relative);
                    if path.is_absolute()
                        || path
                            .components()
                            .any(|c| !matches!(c, std::path::Component::Normal(_)))
                    {
                        return Err(Error::InvalidPath(relative.into()));
                    }
                    blocked_sources.push(root.join(path));
                }
                if deps.contains_key(&name) {
                    return Err(Error::InvalidOperation(
                        "duplicate runtime/dev dependency".into(),
                    ));
                }
                registry.entry(name).or_insert(source);
                continue;
            }
            if deps.insert(name.clone(), source).is_some() {
                return Err(Error::InvalidOperation(format!(
                    "duplicate runtime/dev dependency {name}"
                )));
            }
        }
        let source_root = inside(&root, &source_root.unwrap_or_else(|| "src".into()))?;
        if !source_root.is_dir() {
            return Err(Error::InvalidPath(source_root.display().to_string()));
        }
        let entry = entry.unwrap_or_else(|| "main.rw".into());
        let entry = inside(&source_root, &entry)?;
        let mut imports = BTreeMap::new();
        imports.insert(String::new(), source_root.clone());
        if matches!(
            language.as_str(),
            "0.6"
                | "0.7"
                | "0.8"
                | "0.9"
                | "0.9.1"
                | "0.9.2"
                | "0.9.3"
                | "0.9.4"
                | "0.9.5"
                | "0.9.6"
                | "0.9.7"
                | "0.9.8"
                | "0.9.9"
                | "1.0.0"
                | "1.1.0"
                | "1.2.0"
                | "1.3.0"
                | "1.4.0"
                | "1.5.0"
                | "1.6.0"
                | "1.6.1"
                | "1.7.0"
                | "1.7.1"
                | "1.8.0"
                | "1.8.1"
                | "1.8.2"
                | "1.8.3"
                | "1.8.4"
                | "1.8.5"
                | "1.8.6"
                | "1.8.7"
                | "1.8.8"
                | "1.9.0"
                | "1.9.1"
                | "1.9.2"
                | "1.9.3"
                | "1.9.4"
                | "1.9.5"
                | "1.9.6"
                | "1.9.7"
                | "1.9.8"
                | "1.9.9"
                | "1.9.10"
                | "1.9.11"
                | "1.9.12"
                | "1.9.13"
                | "1.9.14"
                | "1.9.15"
                | "1.9.16"
                | "1.9.17"
                | "1.9.18"
                | "1.9.19"
                | "1.9.20"
                | "1.9.21"
                | "1.9.22"
                | "1.9.23"
                | "1.9.24"
                | "1.9.25"
                | "1.9.26"
                | "1.9.27"
                | "1.9.28"
                | "1.9.29"
                | "1.9.30"
                | "1.9.31"
                | "1.9.32"
                | "1.9.33"
                | "1.9.34"
                | "1.9.35"
                | "1.9.36"
                | "1.9.37"
                | "1.9.38"
                | "1.9.39"
                | "1.9.40"
                | "1.9.41"
                | "1.9.42"
                | "1.9.43"
                | "1.9.44"
                | "1.9.45"
                | "1.9.46"
                | "1.9.47"
                | "1.9.48"
                | "2.0.0"
        ) && deps
            .values()
            .chain(registry.values())
            .any(|s| s.contains('|'))
        {
            let (selected, constraints) = v06::resolver::resolve(
                &root, &deps, &registry, &versions, &signers, &trust, &revoked, latest,
            )?;
            imports.extend(selected);
            versions = constraints;
        } else {
            for (name, path) in deps {
                if name.contains('/') || name.contains('.') || name.is_empty() {
                    return Err(Error::InvalidOperation(format!(
                        "invalid dependency name {name}"
                    )));
                }
                let full = inside(&root, path.strip_prefix("file:").unwrap_or(&path))?;
                if !full.is_dir() {
                    return Err(Error::InvalidPath(path));
                }
                imports.insert(name, full);
            }
            if matches!(
                language.as_str(),
                "0.5"
                    | "0.6"
                    | "0.7"
                    | "0.8"
                    | "0.9"
                    | "0.9.1"
                    | "0.9.2"
                    | "0.9.3"
                    | "0.9.4"
                    | "0.9.5"
                    | "0.9.6"
                    | "0.9.7"
                    | "0.9.8"
                    | "0.9.9"
                    | "1.0.0"
                    | "1.1.0"
                    | "1.2.0"
                    | "1.3.0"
                    | "1.4.0"
                    | "1.5.0"
                    | "1.6.0"
                    | "1.6.1"
                    | "1.7.0"
                    | "1.7.1"
                    | "1.8.0"
                    | "1.8.1"
                    | "1.8.2"
                    | "1.8.3"
                    | "1.8.4"
                    | "1.8.5"
                    | "1.8.6"
                    | "1.8.7"
                    | "1.8.8"
                    | "1.9.0"
                    | "1.9.1"
                    | "1.9.2"
                    | "1.9.3"
                    | "1.9.4"
                    | "1.9.5"
                    | "1.9.6"
                    | "1.9.7"
                    | "1.9.8"
                    | "1.9.9"
                    | "1.9.10"
                    | "1.9.11"
                    | "1.9.12"
                    | "1.9.13"
                    | "1.9.14"
                    | "1.9.15"
                    | "1.9.16"
                    | "1.9.17"
                    | "1.9.18"
                    | "1.9.19"
                    | "1.9.20"
                    | "1.9.21"
                    | "1.9.22"
                    | "1.9.23"
                    | "1.9.24"
                    | "1.9.25"
                    | "1.9.26"
                    | "1.9.27"
                    | "1.9.28"
                    | "1.9.29"
                    | "1.9.30"
                    | "1.9.31"
                    | "1.9.32"
                    | "1.9.33"
                    | "1.9.34"
                    | "1.9.35"
                    | "1.9.36"
                    | "1.9.37"
                    | "1.9.38"
                    | "1.9.39"
                    | "1.9.40"
                    | "1.9.41"
                    | "1.9.42"
                    | "1.9.43"
                    | "1.9.44"
                    | "1.9.45"
                    | "1.9.46"
                    | "1.9.47"
                    | "1.9.48"
                    | "2.0.0"
            ) {
                let mut inspected = BTreeSet::new();
                loop {
                    let pending = imports
                        .iter()
                        .filter(|(n, _)| !n.is_empty() && !inspected.contains(*n))
                        .map(|(n, p)| (n.clone(), p.clone()))
                        .collect::<Vec<_>>();
                    if pending.is_empty() {
                        break;
                    }
                    for (name, path) in pending {
                        inspected.insert(name);
                        let metadata: serde_json::Value =
                            serde_json::from_slice(&fs::read(path.join("rewind.package.json"))?)
                                .map_err(|e| Error::InvalidOperation(e.to_string()))?;
                        if let Some(dependencies) = metadata.get("dependencies") {
                            let dependencies = dependencies.as_object().ok_or_else(|| {
                                Error::InvalidOperation(
                                    "package dependencies must be an object".into(),
                                )
                            })?;
                            for (name, wanted) in dependencies {
                                let wanted = wanted.as_str().ok_or_else(|| {
                                    Error::InvalidOperation(
                                        "transitive requirement must be a string".into(),
                                    )
                                })?;
                                semver::VersionReq::parse(wanted)
                                    .map_err(|e| Error::InvalidOperation(e.to_string()))?;
                                versions
                                    .entry(name.clone())
                                    .and_modify(|v| {
                                        v.push_str(", ");
                                        v.push_str(wanted);
                                    })
                                    .or_insert_with(|| wanted.into());
                                if !imports.contains_key(name) {
                                    let source=registry.get(name).ok_or_else(||Error::InvalidOperation(format!("transitive package {name} requires an explicit registry mirror")))?;
                                    imports.insert(
                                        name.clone(),
                                        inside(
                                            &root,
                                            source.strip_prefix("file:").unwrap_or(source),
                                        )?,
                                    );
                                    if imports.len() > 1024 {
                                        return Err(Error::InvalidOperation(
                                            "DependencyBudgetExceeded".into(),
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        let lock_imports = imports.clone();
        if matches!(
            language.as_str(),
            "0.8"
                | "0.9"
                | "0.9.1"
                | "0.9.2"
                | "0.9.3"
                | "0.9.4"
                | "0.9.5"
                | "0.9.6"
                | "0.9.7"
                | "0.9.8"
                | "0.9.9"
                | "1.0.0"
                | "1.1.0"
                | "1.2.0"
                | "1.3.0"
                | "1.4.0"
                | "1.5.0"
                | "1.6.0"
                | "1.6.1"
                | "1.7.0"
                | "1.7.1"
                | "1.8.0"
                | "1.8.1"
                | "1.8.2"
                | "1.8.3"
                | "1.8.4"
                | "1.8.5"
                | "1.8.6"
                | "1.8.7"
                | "1.8.8"
                | "1.9.0"
                | "1.9.1"
                | "1.9.2"
                | "1.9.3"
                | "1.9.4"
                | "1.9.5"
                | "1.9.6"
                | "1.9.7"
                | "1.9.8"
                | "1.9.9"
                | "1.9.10"
                | "1.9.11"
                | "1.9.12"
                | "1.9.13"
                | "1.9.14"
                | "1.9.15"
                | "1.9.16"
                | "1.9.17"
                | "1.9.18"
                | "1.9.19"
                | "1.9.20"
                | "1.9.21"
                | "1.9.22"
                | "1.9.23"
                | "1.9.24"
                | "1.9.25"
                | "1.9.26"
                | "1.9.27"
                | "1.9.28"
                | "1.9.29"
                | "1.9.30"
                | "1.9.31"
                | "1.9.32"
                | "1.9.33"
                | "1.9.34"
                | "1.9.35"
                | "1.9.36"
                | "1.9.37"
                | "1.9.38"
                | "1.9.39"
                | "1.9.40"
                | "1.9.41"
                | "1.9.42"
                | "1.9.43"
                | "1.9.44"
                | "1.9.45"
                | "1.9.46"
                | "1.9.47"
                | "1.9.48"
                | "2.0.0"
        ) && !include_dev
        {
            let mut reachable = runtime_roots;
            let mut pending = reachable.iter().cloned().collect::<Vec<_>>();
            while let Some(name) = pending.pop() {
                let path = imports
                    .get(&name)
                    .ok_or_else(|| Error::InvalidOperation(format!("missing dependency {name}")))?;
                let metadata: serde_json::Value =
                    serde_json::from_slice(&fs::read(path.join("rewind.package.json"))?)
                        .map_err(|e| Error::InvalidOperation(e.to_string()))?;
                if let Some(deps) = metadata["dependencies"].as_object() {
                    for name in deps.keys() {
                        if reachable.insert(name.clone()) {
                            pending.push(name.clone());
                        }
                    }
                }
            }
            imports.retain(|name, _| name.is_empty() || reachable.contains(name));
        }
        if production {
            blocked_sources.retain(|p| !imports.values().any(|runtime| runtime == p));
            let lock: serde_json::Value =
                serde_json::from_slice(&fs::read(root.join("rewind.lock"))?)
                    .map_err(|e| Error::InvalidOperation(e.to_string()))?;
            if let Some(entries) = lock["dependencies"].as_object() {
                for (name, entry) in entries {
                    if imports.contains_key(name) {
                        continue;
                    }
                    let relative = entry["source"]
                        .as_str()
                        .ok_or_else(|| Error::InvalidOperation("invalid locked source".into()))?;
                    let path = Path::new(relative);
                    if path.is_absolute()
                        || path
                            .components()
                            .any(|c| !matches!(c, std::path::Component::Normal(_)))
                    {
                        return Err(Error::InvalidPath(relative.into()));
                    }
                    blocked_sources.push(root.join(path));
                }
            }
        }
        Ok(Some(Self {
            production,
            assets,
            blocked_sources,
            lock_imports,
            source_root,
            entry,
            imports,
            language,
            effects,
            versions,
            signers,
            trust,
            revoked,
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
        if matches!(
            self.language.as_str(),
            "0.4"
                | "0.5"
                | "0.6"
                | "0.7"
                | "0.8"
                | "0.9"
                | "0.9.1"
                | "0.9.2"
                | "0.9.3"
                | "0.9.4"
                | "0.9.5"
                | "0.9.6"
                | "0.9.7"
                | "0.9.8"
                | "0.9.9"
                | "1.0.0"
                | "1.1.0"
                | "1.2.0"
                | "1.3.0"
                | "1.4.0"
                | "1.5.0"
                | "1.6.0"
                | "1.6.1"
                | "1.7.0"
                | "1.7.1"
                | "1.8.0"
                | "1.8.1"
                | "1.8.2"
                | "1.8.3"
                | "1.8.4"
                | "1.8.5"
                | "1.8.6"
                | "1.8.7"
                | "1.8.8"
                | "1.9.0"
                | "1.9.1"
                | "1.9.2"
                | "1.9.3"
                | "1.9.4"
                | "1.9.5"
                | "1.9.6"
                | "1.9.7"
                | "1.9.8"
                | "1.9.9"
                | "1.9.10"
                | "1.9.11"
                | "1.9.12"
                | "1.9.13"
                | "1.9.14"
                | "1.9.15"
                | "1.9.16"
                | "1.9.17"
                | "1.9.18"
                | "1.9.19"
                | "1.9.20"
                | "1.9.21"
                | "1.9.22"
                | "1.9.23"
                | "1.9.24"
                | "1.9.25"
                | "1.9.26"
                | "1.9.27"
                | "1.9.28"
                | "1.9.29"
                | "1.9.30"
                | "1.9.31"
                | "1.9.32"
                | "1.9.33"
                | "1.9.34"
                | "1.9.35"
                | "1.9.36"
                | "1.9.37"
                | "1.9.38"
                | "1.9.39"
                | "1.9.40"
                | "1.9.41"
                | "1.9.42"
                | "1.9.43"
                | "1.9.44"
                | "1.9.45"
                | "1.9.46"
                | "1.9.47"
                | "1.9.48"
                | "2.0.0"
        ) {
            return self.secure_lock(root, update);
        }
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
    pub(super) fn execution_lock_hash(&self, root: &Path) -> Result<String> {
        if self.production {
            self.lock(root, false)?;
            Ok(packages::hash(self.lock_document(root)?))
        } else {
            Ok(packages::hash(fs::read(root.join("rewind.lock"))?))
        }
    }
    pub(super) fn lock_document(&self, root: &Path) -> Result<String> {
        use ed25519_dalek::{Signature, VerifyingKey};
        let mut dependencies = serde_json::Map::new();
        for (name, path) in &self.lock_imports {
            if name.is_empty() {
                continue;
            }
            let metadata: serde_json::Value =
                serde_json::from_slice(&fs::read(path.join("rewind.package.json"))?)
                    .map_err(|e| Error::InvalidOperation(format!("package {name}: {e}")))?;
            let version = metadata["version"].as_str().ok_or_else(|| {
                Error::InvalidOperation(format!("package {name}: missing version"))
            })?;
            if metadata["name"].as_str() != Some(name) {
                return Err(Error::InvalidOperation(format!(
                    "package {name}: metadata name mismatch"
                )));
            }
            let wanted = self.versions.get(name).ok_or_else(|| {
                Error::InvalidOperation(format!("package {name}: missing version requirement"))
            })?;
            let requirement = semver::VersionReq::parse(wanted)
                .map_err(|e| Error::InvalidOperation(e.to_string()))?;
            let actual = semver::Version::parse(version)
                .map_err(|e| Error::InvalidOperation(e.to_string()))?;
            if !requirement.matches(&actual) {
                return Err(Error::InvalidOperation(format!(
                    "package {name}: version {version} does not satisfy {wanted}"
                )));
            }
            let signer = self.signers.get(name).ok_or_else(|| {
                Error::InvalidOperation(format!("package {name}: missing signer"))
            })?;
            if self.revoked.contains(signer) {
                return Err(Error::InvalidOperation(format!(
                    "revoked package signer {signer}"
                )));
            }
            let public = self.trust.get(signer).ok_or_else(|| {
                Error::InvalidOperation(format!("untrusted package signer {signer}"))
            })?;
            let key_bytes = super::packages::decode_hex(public)?;
            let key = VerifyingKey::from_bytes(&key_bytes.try_into().map_err(|_| {
                Error::InvalidOperation("Ed25519 public key must have 32 bytes".into())
            })?)
            .map_err(|e| Error::InvalidOperation(e.to_string()))?;
            let hash = super::packages::package_hash(path)?;
            let signature_hex = fs::read_to_string(path.join("rewind.signature"))?;
            let signature =
                Signature::from_slice(&super::packages::decode_hex(signature_hex.trim())?)
                    .map_err(|e| Error::InvalidOperation(e.to_string()))?;
            key.verify_strict(
                super::packages::signature_message(&hash).as_bytes(),
                &signature,
            )
            .map_err(|_| {
                Error::InvalidOperation(format!("package {name}: signature verification failed"))
            })?;
            let needed = metadata["effects"].as_array().ok_or_else(|| {
                Error::InvalidOperation(format!("package {name}: missing effects array"))
            })?;
            for effect in needed {
                let effect = effect.as_str().ok_or_else(|| {
                    Error::InvalidOperation("package effect must be a string".into())
                })?;
                if self.language == "0.4" && !self.effects.contains(effect) {
                    return Err(Error::InvalidOperation(format!(
                        "package {name}: effect {effect} is not allowed"
                    )));
                }
            }
            let mut selected = serde_json::json!({"source":path.strip_prefix(fs::canonicalize(root)?).map_err(|_| Error::InvalidPath(name.clone()))?.to_string_lossy().replace('\\',"/"),"version":version,"sha256":hash,"signer":signer,"public_key":public,"signature":signature_hex.trim()});
            if matches!(
                self.language.as_str(),
                "0.5"
                    | "0.6"
                    | "0.7"
                    | "0.8"
                    | "0.9"
                    | "0.9.1"
                    | "0.9.2"
                    | "0.9.3"
                    | "0.9.4"
                    | "0.9.5"
                    | "0.9.6"
                    | "0.9.7"
                    | "0.9.8"
                    | "0.9.9"
                    | "1.0.0"
                    | "1.1.0"
                    | "1.2.0"
                    | "1.3.0"
                    | "1.4.0"
                    | "1.5.0"
                    | "1.6.0"
                    | "1.6.1"
                    | "1.7.0"
                    | "1.7.1"
                    | "1.8.0"
                    | "1.8.1"
                    | "1.8.2"
                    | "1.8.3"
                    | "1.8.4"
                    | "1.8.5"
                    | "1.8.6"
                    | "1.8.7"
                    | "1.8.8"
                    | "1.9.0"
                    | "1.9.1"
                    | "1.9.2"
                    | "1.9.3"
                    | "1.9.4"
                    | "1.9.5"
                    | "1.9.6"
                    | "1.9.7"
                    | "1.9.8"
                    | "1.9.9"
                    | "1.9.10"
                    | "1.9.11"
                    | "1.9.12"
                    | "1.9.13"
                    | "1.9.14"
                    | "1.9.15"
                    | "1.9.16"
                    | "1.9.17"
                    | "1.9.18"
                    | "1.9.19"
                    | "1.9.20"
                    | "1.9.21"
                    | "1.9.22"
                    | "1.9.23"
                    | "1.9.24"
                    | "1.9.25"
                    | "1.9.26"
                    | "1.9.27"
                    | "1.9.28"
                    | "1.9.29"
                    | "1.9.30"
                    | "1.9.31"
                    | "1.9.32"
                    | "1.9.33"
                    | "1.9.34"
                    | "1.9.35"
                    | "1.9.36"
                    | "1.9.37"
                    | "1.9.38"
                    | "1.9.39"
                    | "1.9.40"
                    | "1.9.41"
                    | "1.9.42"
                    | "1.9.43"
                    | "1.9.44"
                    | "1.9.45"
                    | "1.9.46"
                    | "1.9.47"
                    | "1.9.48"
                    | "2.0.0"
            ) {
                selected["requirement"] = wanted.clone().into();
                selected["dependencies"] = metadata
                    .get("dependencies")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));
                selected["effects"] = metadata["effects"].clone();
            }
            dependencies.insert(name.clone(), selected);
        }
        let mut document = serde_json::json!({"format":2,"language":self.language,"compiler":env!("CARGO_PKG_VERSION"),"effects":self.effects,"dependencies":dependencies});
        if !self.assets.is_empty() {
            if !matches!(
                self.language.as_str(),
                "0.9.1"
                    | "0.9.2"
                    | "0.9.3"
                    | "0.9.4"
                    | "0.9.5"
                    | "0.9.6"
                    | "0.9.7"
                    | "0.9.8"
                    | "0.9.9"
                    | "1.0.0"
                    | "1.1.0"
                    | "1.2.0"
                    | "1.3.0"
                    | "1.4.0"
                    | "1.5.0"
                    | "1.6.0"
                    | "1.6.1"
                    | "1.7.0"
                    | "1.7.1"
                    | "1.8.0"
                    | "1.8.1"
                    | "1.8.2"
                    | "1.8.3"
                    | "1.8.4"
                    | "1.8.5"
                    | "1.8.6"
                    | "1.8.7"
                    | "1.8.8"
                    | "1.9.0"
                    | "1.9.1"
                    | "1.9.2"
                    | "1.9.3"
                    | "1.9.4"
                    | "1.9.5"
                    | "1.9.6"
                    | "1.9.7"
                    | "1.9.8"
                    | "1.9.9"
                    | "1.9.10"
                    | "1.9.11"
                    | "1.9.12"
                    | "1.9.13"
                    | "1.9.14"
                    | "1.9.15"
                    | "1.9.16"
                    | "1.9.17"
                    | "1.9.18"
                    | "1.9.19"
                    | "1.9.20"
                    | "1.9.21"
                    | "1.9.22"
                    | "1.9.23"
                    | "1.9.24"
                    | "1.9.25"
                    | "1.9.26"
                    | "1.9.27"
                    | "1.9.28"
                    | "1.9.29"
                    | "1.9.30"
                    | "1.9.31"
                    | "1.9.32"
                    | "1.9.33"
                    | "1.9.34"
                    | "1.9.35"
                    | "1.9.36"
                    | "1.9.37"
                    | "1.9.38"
                    | "1.9.39"
                    | "1.9.40"
                    | "1.9.41"
                    | "1.9.42"
                    | "1.9.43"
                    | "1.9.44"
                    | "1.9.45"
                    | "1.9.46"
                    | "1.9.47"
                    | "1.9.48"
                    | "2.0.0"
            ) {
                return Err(Error::InvalidOperation(
                    "assets require language 0.9.1".into(),
                ));
            }
            document["assets"] = v091::asset_inventory(root, &self.assets)?;
        }
        let expected = serde_json::to_string_pretty(&document)
            .map_err(|e| Error::InvalidOperation(e.to_string()))?
            + "\n";
        Ok(expected)
    }
    fn secure_lock(&self, root: &Path, update: bool) -> Result<()> {
        let expected = self.lock_document(root)?;
        if self.production {
            let actual: serde_json::Value =
                serde_json::from_slice(&fs::read(root.join("rewind.lock"))?)
                    .map_err(|e| Error::InvalidOperation(e.to_string()))?;
            let verified: serde_json::Value = serde_json::from_str(&expected)
                .map_err(|e| Error::InvalidOperation(e.to_string()))?;
            if ["format", "language", "compiler", "effects", "assets"]
                .iter()
                .any(|key| actual[key] != verified[key])
                || verified["dependencies"]
                    .as_object()
                    .unwrap()
                    .iter()
                    .any(|(name, entry)| actual["dependencies"][name] != *entry)
            {
                return Err(Error::InvalidOperation(
                    "production runtime lock mismatch".into(),
                ));
            }
            return Ok(());
        }
        let parsed: serde_json::Value =
            serde_json::from_str(&expected).map_err(|e| Error::InvalidOperation(e.to_string()))?;
        let dependencies = parsed["dependencies"].as_object().unwrap();
        let path = root.join("rewind.lock");
        if update {
            if matches!(
                self.language.as_str(),
                "0.5"
                    | "0.6"
                    | "0.7"
                    | "0.8"
                    | "0.9"
                    | "0.9.1"
                    | "0.9.2"
                    | "0.9.3"
                    | "0.9.4"
                    | "0.9.5"
                    | "0.9.6"
                    | "0.9.7"
                    | "0.9.8"
                    | "0.9.9"
                    | "1.0.0"
                    | "1.1.0"
                    | "1.2.0"
                    | "1.3.0"
                    | "1.4.0"
                    | "1.5.0"
                    | "1.6.0"
                    | "1.6.1"
                    | "1.7.0"
                    | "1.7.1"
                    | "1.8.0"
                    | "1.8.1"
                    | "1.8.2"
                    | "1.8.3"
                    | "1.8.4"
                    | "1.8.5"
                    | "1.8.6"
                    | "1.8.7"
                    | "1.8.8"
                    | "1.9.0"
                    | "1.9.1"
                    | "1.9.2"
                    | "1.9.3"
                    | "1.9.4"
                    | "1.9.5"
                    | "1.9.6"
                    | "1.9.7"
                    | "1.9.8"
                    | "1.9.9"
                    | "1.9.10"
                    | "1.9.11"
                    | "1.9.12"
                    | "1.9.13"
                    | "1.9.14"
                    | "1.9.15"
                    | "1.9.16"
                    | "1.9.17"
                    | "1.9.18"
                    | "1.9.19"
                    | "1.9.20"
                    | "1.9.21"
                    | "1.9.22"
                    | "1.9.23"
                    | "1.9.24"
                    | "1.9.25"
                    | "1.9.26"
                    | "1.9.27"
                    | "1.9.28"
                    | "1.9.29"
                    | "1.9.30"
                    | "1.9.31"
                    | "1.9.32"
                    | "1.9.33"
                    | "1.9.34"
                    | "1.9.35"
                    | "1.9.36"
                    | "1.9.37"
                    | "1.9.38"
                    | "1.9.39"
                    | "1.9.40"
                    | "1.9.41"
                    | "1.9.42"
                    | "1.9.43"
                    | "1.9.44"
                    | "1.9.45"
                    | "1.9.46"
                    | "1.9.47"
                    | "1.9.48"
                    | "2.0.0"
            ) {
                let old: serde_json::Value = fs::read(&path)
                    .ok()
                    .and_then(|b| serde_json::from_slice(&b).ok())
                    .unwrap_or(serde_json::Value::Null);
                for (name, value) in dependencies {
                    if old["dependencies"][name] != *value {
                        eprintln!(
                            "selected {name}: {} -> {}; satisfies {}; explicit mirror {}",
                            old["dependencies"][name]["version"]
                                .as_str()
                                .unwrap_or("(none)"),
                            value["version"].as_str().unwrap_or("?"),
                            value["requirement"].as_str().unwrap_or("?"),
                            value["source"].as_str().unwrap_or("?")
                        );
                    }
                }
                if let Some(old) = old["dependencies"].as_object() {
                    for name in old.keys() {
                        if !dependencies.contains_key(name) {
                            eprintln!("removed {name}");
                        }
                    }
                }
            }
            fs::write(path, expected)?;
        } else if !path.exists() {
            return Err(Error::InvalidOperation(
                "v0.4 requires rewind.lock; run 'rewind update' explicitly".into(),
            ));
        } else if fs::read_to_string(path)? != expected {
            return Err(Error::InvalidOperation(
                "rewind.lock mismatch; run 'rewind update' explicitly".into(),
            ));
        }
        Ok(())
    }
}

/// Runtime policy for a compiled program: no source graph or package files are opened.
pub(super) struct RuntimePolicy {
    pub language: String,
    pub effects: BTreeSet<String>,
    pub assets: serde_json::Value,
}
impl RuntimePolicy {
    pub fn load(root: &Path) -> Result<Option<Self>> {
        let root = fs::canonicalize(root)?;
        let Some(manifest) = Manifest::load(&root)? else {
            return Ok(None);
        };
        // Source paths still obey the manifest grammar, but may be absent in a distribution.
        for relative in [
            manifest.source_root.as_deref().unwrap_or("src"),
            manifest.entry.as_deref().unwrap_or("main.rw"),
        ] {
            let path = Path::new(relative);
            if relative != "."
                && (path.as_os_str().is_empty()
                    || path.is_absolute()
                    || path
                        .components()
                        .any(|c| !matches!(c, std::path::Component::Normal(_))))
            {
                return Err(Error::InvalidPath(relative.into()));
            }
        }
        let assets = v091::asset_inventory(&root, &manifest.assets)?;
        let lock = root.join("rewind.lock");
        if !lock.is_file() {
            return Err(Error::InvalidOperation(
                "compiled runtime requires rewind.lock".into(),
            ));
        }
        if fs::metadata(&lock)?.len() > 32 * 1024 * 1024 {
            return Err(Error::InvalidOperation(
                "compiled runtime lock exceeds 32 MiB".into(),
            ));
        }
        let actual: serde_json::Value = serde_json::from_slice(&fs::read(lock)?)
            .map_err(|e| Error::InvalidOperation(format!("invalid runtime lock: {e}")))?;
        let expected_effects = serde_json::to_value(&manifest.effects)
            .map_err(|e| Error::InvalidOperation(e.to_string()))?;
        let expected_assets = if manifest.assets.is_empty() {
            serde_json::Value::Null
        } else {
            assets.clone()
        };
        if actual["format"] != 2
            || actual["language"] != manifest.language
            || actual["compiler"] != env!("CARGO_PKG_VERSION")
            || actual["effects"] != expected_effects
            || actual["assets"] != expected_assets
            || !actual["dependencies"].is_object()
        {
            return Err(Error::InvalidOperation(
                "compiled runtime lock mismatch".into(),
            ));
        }
        Ok(Some(Self {
            language: manifest.language,
            effects: manifest.effects,
            assets,
        }))
    }
}
