use super::*;
use std::io::{Read, Write};

struct Session<'a> {
    root: &'a Path,
    config: &'a project::ProjectConfig,
    accepted: String,
    previous: Option<Program>,
    tape: Option<Json>,
    state: Json,
    pending: Option<String>,
    epoch: usize,
    steps: usize,
}
impl<'a> Session<'a> {
    fn new(root: &'a Path, config: &'a project::ProjectConfig) -> Self {
        Self {
            root,
            config,
            accepted: String::new(),
            previous: None,
            tape: None,
            state: json!({}),
            pending: None,
            epoch: 0,
            steps: 0,
        }
    }
    fn input(&mut self, line: &str, replay: Option<&Json>, recording: bool) -> Result<Json> {
        if line.len() > 64 * 1024 {
            return Err(Error::InvalidOperation("REPL input exceeds 64 KiB".into()));
        }
        let text = line.trim();
        if text == ":quit" {
            if self.pending.is_some() {
                return Err(Error::InvalidOperation(
                    "unfinished multiline input; use :end or :cancel".into(),
                ));
            }
            return Ok(json!({"quit":true}));
        }
        if text == ":state" {
            return Ok(json!({"state":self.state,"pending":self.pending.is_some()}));
        }
        if text == ":begin" {
            if self.pending.is_some() {
                return Err(Error::InvalidOperation("nested multiline input".into()));
            }
            self.pending = Some(String::new());
            return Ok(json!({"pending":true,"state":self.state}));
        }
        if text == ":cancel" {
            self.pending = None;
            return Ok(json!({"cancelled":true,"state":self.state}));
        }
        if text == ":reset" {
            self.accepted.clear();
            self.previous = None;
            self.tape = None;
            self.state = json!({});
            self.pending = None;
            self.epoch += 1;
            return Ok(json!({"ok":true,"reset":true,"epoch":self.epoch}));
        }
        let input = if text == ":end" {
            self.pending
                .take()
                .ok_or_else(|| Error::InvalidOperation(":end requires :begin".into()))?
        } else if let Some(pending) = &mut self.pending {
            pending.push_str(line);
            if pending.len() > 64 * 1024 {
                return Err(Error::InvalidOperation(
                    "multiline input exceeds 64 KiB".into(),
                ));
            }
            return Ok(json!({"pending":true,"state":self.state}));
        } else {
            line.to_string()
        };
        let candidate = format!("{}{input}", self.accepted);
        // Source and ordinary observations are part of a transcript. Secret reinjection needs a separate protocol.
        if recording
            && lex(&candidate)
                .map(|tokens| {
                    tokens.iter().any(|t| {
                        matches!(
                            t.text.as_str(),
                            "secret" | "Secret" | "reveal" | "readSecretLine" | "getSecret"
                        )
                    })
                })
                .unwrap_or_else(|_| {
                    candidate.contains("secret")
                        || candidate.contains("Secret")
                        || candidate.contains("reveal")
                })
        {
            return Err(Error::InvalidOperation(
                "session transcripts do not support Secret inputs or reveal".into(),
            ));
        }
        let result = (|| {
            if candidate.len() > 1024 * 1024 {
                return Err(Error::InvalidOperation("REPL source exceeds 1 MiB".into()));
            }
            if self.steps >= 1_000_000 {
                return Err(Error::InvalidOperation(
                    "SessionExecutionBudgetExceeded".into(),
                ));
            }
            let program = v08::session_program(self.root, self.config, &candidate)?;
            if recording {
                let mut secret = false;
                let mut inspect = |e: &Expr| {
                    if let ExprKind::Name(n) = &e.kind {
                        secret |= n.contains("Secret") || matches!(n.as_str(), "secret" | "reveal");
                    }
                    if let ExprKind::Member(_, n) = &e.kind {
                        secret |= n.contains("Secret");
                    }
                };
                v05::expressions(&program.stmts, &mut inspect);
                for f in program.functions.values() {
                    v05::expressions(&f.body, &mut inspect);
                }
                if secret {
                    return Err(Error::InvalidOperation(
                        "session transcripts do not support Secret APIs in imported code".into(),
                    ));
                }
            }
            if let Some(old) = self.previous.as_ref().filter(|p| !p.stmts.is_empty()) {
                if serde_json::to_value((
                    &old.functions,
                    &old.structs,
                    &old.enums,
                    &old.traits,
                    &old.impls,
                    &old.aliases,
                ))
                .ok()
                    != serde_json::to_value((
                        &program.functions,
                        &program.structs,
                        &program.enums,
                        &program.traits,
                        &program.impls,
                        &program.aliases,
                    ))
                    .ok()
                {
                    return Err(Error::InvalidOperation(
                        "REPL declarations must precede executable inputs".into(),
                    ));
                }
            }
            let tape = replay.filter(|v| !v.is_null()).or(self.tape.as_ref());
            let (outcome, tape, state, steps) = vm::session_mode(
                program.clone(),
                self.root,
                tape,
                replay.is_some(),
                100_000.min(1_000_000 - self.steps),
            )?;
            self.steps += steps;
            self.tape = Some(tape);
            outcome?;
            self.accepted = candidate;
            self.previous = Some(program);
            self.state = state;
            Ok(())
        })();
        Ok(match result {
            Ok(()) => {
                json!({"ok":true,"state":self.state,"epoch":self.epoch,"session_steps":self.steps})
            }
            Err(e) => {
                json!({"ok":false,"state":self.state,"epoch":self.epoch,"session_steps":self.steps,"error":e.to_string(),"diagnostic":if let Error::Diagnostic(d)=&e{Some(d.as_ref())}else{None}})
            }
        })
    }
}
fn context(root: &Path, config: &project::ProjectConfig) -> Result<Json> {
    let mut sources = BTreeMap::new();
    fn hash_tree(path: &Path, root: &Path, values: &mut BTreeMap<String, String>) -> Result<()> {
        for entry in fs::read_dir(path)? {
            let path = entry?.path();
            if fs::symlink_metadata(&path)?.file_type().is_symlink() {
                return Err(Error::InvalidPath("session source symlink".into()));
            }
            if path
                .file_name()
                .is_some_and(|s| s == ".rewind" || s == ".git" || s == "target")
            {
                continue;
            }
            if path.is_dir() {
                hash_tree(&path, root, values)?;
            } else if path.extension().is_some_and(|s| s == "rw") {
                values.insert(
                    path.strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .into_owned(),
                    packages::hash(fs::read(path)?),
                );
            }
        }
        Ok(())
    }
    hash_tree(&config.source_root, root, &mut sources)?;
    Ok(
        json!({"root":root,"manifest":packages::hash(fs::read(root.join("rewind.toml"))?),"lock":config.execution_lock_hash(root)?,"sources":sources}),
    )
}
pub(in crate::v2) fn repl(root: &Path, record: Option<&Path>) -> Result<()> {
    let root = fs::canonicalize(root)?;
    let config = project::ProjectConfig::load(&root)?
        .ok_or_else(|| Error::InvalidOperation("repl requires a manifest".into()))?;
    config.lock(&root, false)?;
    let context = if record.is_some() {
        Some(context(&root, &config)?)
    } else {
        None
    };
    let mut session = Session::new(&root, &config);
    let mut entries = Vec::new();
    let mut bytes = 0;
    let stdin = io::stdin();
    let mut input = io::BufReader::new(stdin.lock().take(1024 * 1024 + 1));
    let stdout = io::stdout();
    let mut output = stdout.lock();
    for index in 0..256 {
        let mut line = String::new();
        if input.read_line(&mut line)? == 0 {
            break;
        }
        let response = session.input(&line, None, record.is_some())?;
        writeln!(output, "{}", json!({"input":index,"response":response}))?;
        output.flush()?;
        if record.is_some() {
            let entry = json!({"input":line,"response":response,"observations":session.tape});
            bytes += serde_json::to_vec(&entry)
                .map_err(|e| Error::InvalidOperation(e.to_string()))?
                .len();
            if bytes > 32 * 1024 * 1024 {
                return Err(Error::InvalidOperation(
                    "session transcript exceeds 32 MiB".into(),
                ));
            }
            entries.push(entry);
        }
        if response["quit"] == true {
            break;
        }
        if index == 255 {
            return Err(Error::InvalidOperation(
                "REPL input budget exceeded (256 lines)".into(),
            ));
        }
    }
    if session.pending.is_some() {
        return Err(Error::InvalidOperation(
            "unfinished multiline input; use :end or :cancel".into(),
        ));
    }
    if let Some(path) = record {
        let transcript = json!({"format":1,"kind":"rewind-session","compiler":env!("CARGO_PKG_VERSION"),"context":context,"entries":entries,"final_state":session.state,"epoch":session.epoch,"session_steps":session.steps});
        if context.as_ref() != Some(&self::context(&root, &config)?) {
            return Err(Error::InvalidOperation(
                "session sources changed during recording".into(),
            ));
        }
        let bytes = serde_json::to_vec_pretty(&transcript)
            .map_err(|e| Error::InvalidOperation(e.to_string()))?;
        if bytes.len() > 32 * 1024 * 1024 {
            return Err(Error::InvalidOperation(
                "session transcript exceeds 32 MiB".into(),
            ));
        }
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        file.write_all(&bytes)?;
    }
    Ok(())
}
pub(in crate::v2) fn session_replay(root: &Path, path: &Path) -> Result<()> {
    if fs::metadata(path)?.len() > 32 * 1024 * 1024 {
        return Err(Error::InvalidOperation(
            "session transcript exceeds budget".into(),
        ));
    }
    let data: Json = serde_json::from_slice(&fs::read(path)?)
        .map_err(|e| Error::InvalidOperation(e.to_string()))?;
    if data["format"] != 1
        || data["kind"] != "rewind-session"
        || data["compiler"] != env!("CARGO_PKG_VERSION")
    {
        return Err(Error::InvalidOperation(
            "SessionReplayMismatch: format/compiler".into(),
        ));
    }
    let root = fs::canonicalize(root)?;
    let config = project::ProjectConfig::load(&root)?
        .ok_or_else(|| Error::InvalidOperation("session requires manifest".into()))?;
    config.lock(&root, false)?;
    if data["context"] != context(&root, &config)? {
        return Err(Error::InvalidOperation(
            "SessionReplayMismatch: source/manifest/lock".into(),
        ));
    }
    let entries = data["entries"]
        .as_array()
        .filter(|a| a.len() <= 256)
        .ok_or_else(|| Error::InvalidOperation("session entry budget exceeded".into()))?;
    let mut session = Session::new(&root, &config);
    for (index, entry) in entries.iter().enumerate() {
        let line = entry["input"]
            .as_str()
            .ok_or_else(|| Error::InvalidOperation("missing session input".into()))?;
        let response = session.input(line, Some(&entry["observations"]), true)?;
        if response != entry["response"] || json!(session.tape) != entry["observations"] {
            return Err(Error::InvalidOperation(format!(
                "SessionReplayMismatch: entry {index}"
            )));
        }
        if response["quit"] == true && index + 1 != entries.len() {
            return Err(Error::InvalidOperation("session entries after quit".into()));
        }
    }
    if session.pending.is_some()
        || session.state != data["final_state"]
        || data["epoch"] != session.epoch
        || data["session_steps"] != session.steps
    {
        return Err(Error::InvalidOperation(
            "SessionReplayMismatch: final state".into(),
        ));
    }
    println!(
        "{}",
        json!({"ok":true,"entries":entries.len(),"state":session.state,"session_steps":session.steps})
    );
    Ok(())
}
