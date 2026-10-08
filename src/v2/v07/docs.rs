use super::*;
pub(in crate::v2) fn doctest(root: &Path, path: &Path) -> Result<()> {
    if fs::metadata(path)?.len() > 1024 * 1024 {
        return Err(Error::InvalidOperation(
            "documentation exceeds 1 MiB".into(),
        ));
    }
    let config = project::ProjectConfig::load_for_test(root)?
        .ok_or_else(|| Error::InvalidOperation("doctest requires a manifest".into()))?;
    config.lock(root, false)?;
    if !matches!(
        config.language.as_str(),
        "0.7"
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
            | "2.0.0"
    ) {
        return Err(Error::InvalidOperation(
            "doctest requires language 0.7".into(),
        ));
    }
    let root = fs::canonicalize(root)?;
    let mut active = false;
    let mut source = String::new();
    let mut count = 0;
    for (line, text) in fs::read_to_string(path)?.lines().enumerate() {
        if text.trim() == "```rewind" {
            if active {
                return Err(Error::InvalidOperation("nested rewind fence".into()));
            }
            active = true;
            source.clear();
            continue;
        }
        if active && text.trim() == "```" {
            count += 1;
            if count > 256 {
                return Err(Error::InvalidOperation(
                    "doctest example budget exceeded".into(),
                ));
            }
            let virtual_path = root.join(format!(".rewind-doctest-{count}.rw"));
            let overlay = BTreeMap::from([(virtual_path.clone(), source.clone())]);
            let mut p = load_program_overlay(
                &virtual_path,
                &root,
                &config.imports,
                &mut BTreeSet::new(),
                &mut BTreeMap::new(),
                &overlay,
            )?;
            p.language = config.language.clone();
            p.strict_visibility = true;
            v05::prepare(&mut p)?;
            check_program(&p)?;
            v05::validate(&p, &config)?;
            // The source is already checked; avoid reading a fictitious source file to build a recording fingerprint.
            let options = RunOptions {
                virtual_publish: true,
                artifact: Some(json!({"kind":"checked-doctest","example":count})),
                ..RunOptions::default()
            };
            vm::execute(p, &root, false, "run", options).map_err(|e| {
                Error::InvalidOperation(format!("{}:{line}: example {count}: {e}", path.display()))
            })?;
            active = false;
        } else if active {
            source.push_str(text);
            source.push('\n');
        }
    }
    if active {
        return Err(Error::InvalidOperation("unterminated rewind fence".into()));
    }
    println!("{}", json!({"examples":count,"ok":true}));
    Ok(())
}
