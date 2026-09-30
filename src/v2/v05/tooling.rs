use super::*;
use serde_json::{json, Value as Json};
use std::io::{Read, Write};
fn uri_path(uri: &str) -> Result<PathBuf> {
    let text = uri
        .strip_prefix("file://")
        .ok_or_else(|| Error::InvalidOperation("LSP requires file URI".into()))?;
    let mut bytes = Vec::new();
    let raw = text.as_bytes();
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == b'%' {
            let hex = text
                .get(i + 1..i + 3)
                .ok_or_else(|| Error::InvalidPath(uri.into()))?;
            bytes.push(u8::from_str_radix(hex, 16).map_err(|_| Error::InvalidPath(uri.into()))?);
            i += 3;
        } else {
            bytes.push(raw[i]);
            i += 1;
        }
    }
    let text = String::from_utf8(bytes).map_err(|_| Error::InvalidPath(uri.into()))?;
    #[cfg(windows)]
    let text = text.strip_prefix('/').unwrap_or(&text).to_string();
    Ok(PathBuf::from(text))
}
fn file_uri(path: &Path) -> String {
    format!(
        "file:///{}",
        path.to_string_lossy()
            .replace('\\', "/")
            .trim_start_matches('/')
            .replace('%', "%25")
            .replace(' ', "%20")
            .replace('#', "%23")
    )
}
fn document_path(uri: &str) -> Result<PathBuf> {
    let path = uri_path(uri)?;
    if path.exists() {
        return Ok(fs::canonicalize(path)?);
    }
    let parent = fs::canonicalize(
        path.parent()
            .ok_or_else(|| Error::InvalidPath(uri.into()))?,
    )?;
    Ok(parent.join(
        path.file_name()
            .ok_or_else(|| Error::InvalidPath(uri.into()))?,
    ))
}
fn utf16_column(source: &str, line: usize, column: usize) -> usize {
    source
        .lines()
        .nth(line)
        .unwrap_or("")
        .chars()
        .take(column)
        .map(char::len_utf16)
        .sum()
}
fn program(root: &Path, path: &Path, documents: &BTreeMap<PathBuf, String>) -> Result<Program> {
    let config = project::ProjectConfig::load(root)?;
    let mut imports = BTreeMap::from([(String::new(), fs::canonicalize(root)?)]);
    if let Some(c) = &config {
        imports.extend(c.imports.clone());
    }
    let mut program = load_program_overlay(
        path,
        root,
        &imports,
        &mut BTreeSet::new(),
        &mut BTreeMap::new(),
        documents,
    )?;
    program.language = config
        .as_ref()
        .map(|c| c.language.clone())
        .unwrap_or_default();
    program.strict_visibility = !program.language.is_empty() && program.language != "0.2";
    prepare(&mut program)?;
    check_program(&program)?;
    if let Some(c) = &config {
        if c.language == "0.5" {
            validate(&program, c)?;
        } else if c.language == "0.4" {
            effects::validate(&program, c)?;
        }
    }
    Ok(program)
}
fn send(writer: &mut impl Write, value: &Json) -> Result<()> {
    let bytes = serde_json::to_vec(value).map_err(|e| Error::InvalidOperation(e.to_string()))?;
    write!(writer, "Content-Length: {}\r\n\r\n", bytes.len())?;
    writer.write_all(&bytes)?;
    writer.flush()?;
    Ok(())
}
pub fn lsp(root: &Path) -> Result<()> {
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let stdout = io::stdout();
    let mut output = stdout.lock();
    let mut documents = BTreeMap::new();
    let mut shutdown = false;
    loop {
        let mut length = None;
        for _ in 0..16 {
            let mut header = String::new();
            if input.read_line(&mut header)? == 0 {
                return Ok(());
            }
            if header.len() > 4096 {
                return Err(Error::InvalidOperation("LSP header budget exceeded".into()));
            }
            if header.trim().is_empty() {
                break;
            }
            if let Some(n) = header.strip_prefix("Content-Length:") {
                length = Some(
                    n.trim()
                        .parse::<usize>()
                        .map_err(|_| Error::InvalidOperation("invalid LSP length".into()))?,
                );
            }
        }
        let length = length
            .filter(|n| *n <= 1024 * 1024)
            .ok_or_else(|| Error::InvalidOperation("LSP message exceeds 1 MiB".into()))?;
        let mut bytes = vec![0; length];
        input.read_exact(&mut bytes)?;
        let request: Json =
            serde_json::from_slice(&bytes).map_err(|e| Error::InvalidOperation(e.to_string()))?;
        let method = request["method"].as_str().unwrap_or("");
        let id = request.get("id").cloned();
        let params = &request["params"];
        let response = (|| -> Result<Json> {
            match method {
                "initialize" => Ok(
                    json!({"capabilities":{"textDocumentSync":1,"hoverProvider":true,"definitionProvider":true,"completionProvider":{"triggerCharacters":["."]}},"serverInfo":{"name":"REWIND","version":env!("CARGO_PKG_VERSION")}}),
                ),
                "shutdown" => {
                    shutdown = true;
                    Ok(Json::Null)
                }
                "exit" => Ok(Json::Null),
                "textDocument/didOpen" | "textDocument/didChange" | "textDocument/didClose" => {
                    let uri = params["textDocument"]["uri"]
                        .as_str()
                        .ok_or_else(|| Error::InvalidOperation("missing document URI".into()))?;
                    let path = document_path(uri)?;
                    if !path.starts_with(fs::canonicalize(root)?) {
                        return Err(Error::InvalidPath(path.display().to_string()));
                    }
                    if method.ends_with("didClose") {
                        documents.remove(&path);
                        send(
                            &mut output,
                            &json!({"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{"uri":uri,"diagnostics":[]}}),
                        )?;
                        return Ok(Json::Null);
                    }
                    let text = if method.ends_with("didOpen") {
                        params["textDocument"]["text"].as_str()
                    } else {
                        params["contentChanges"][0]["text"].as_str()
                    }
                    .ok_or_else(|| Error::InvalidOperation("full document text required".into()))?;
                    if text.len() > 512 * 1024
                        || documents.len() >= 128 && !documents.contains_key(&path)
                    {
                        return Err(Error::InvalidOperation(
                            "LSP document budget exceeded".into(),
                        ));
                    }
                    documents.insert(path.clone(), text.into());
                    let diagnostics = match program(root, &path, &documents) {
                        Ok(_) => Vec::new(),
                        Err(e) => {
                            let message = e.to_string();
                            let location = lex(text).ok().and_then(|tokens| {
                                tokens
                                    .into_iter()
                                    .find(|t| message.contains(&format!(":{}:{}:", t.line, t.col)))
                            });
                            let line = location.as_ref().map_or(0, |t| t.line.saturating_sub(1));
                            let column = utf16_column(
                                text,
                                line,
                                location.as_ref().map_or(0, |t| t.col.saturating_sub(1)),
                            );
                            vec![
                                json!({"range":{"start":{"line":line,"character":column},"end":{"line":line,"character":column+1}},"severity":1,"source":"rewind","message":message}),
                            ]
                        }
                    };
                    send(
                        &mut output,
                        &json!({"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{"uri":uri,"version":params["textDocument"]["version"],"diagnostics":diagnostics}}),
                    )?;
                    Ok(Json::Null)
                }
                "textDocument/hover" | "textDocument/definition" | "textDocument/completion" => {
                    let path = document_path(params["textDocument"]["uri"].as_str().unwrap_or(""))?;
                    let p = program(root, &path, &documents)?;
                    if method.ends_with("completion") {
                        let mut names = p
                            .functions
                            .keys()
                            .chain(p.structs.keys())
                            .chain(p.enums.keys())
                            .chain(p.traits.keys())
                            .filter(|n| !n.starts_with('$'))
                            .cloned()
                            .collect::<BTreeSet<_>>();
                        names.extend(
                            [
                                "let", "var", "fn", "async", "await", "spawn", "move", "freeze",
                                "thaw", "effects", "for", "match", "commit", "resume",
                            ]
                            .iter()
                            .map(|s| s.to_string()),
                        );
                        return Ok(Json::Array(
                            names
                                .into_iter()
                                .map(|n| json!({"label":n,"kind":3}))
                                .collect(),
                        ));
                    }
                    let source = documents
                        .get(&path)
                        .cloned()
                        .map(Ok)
                        .unwrap_or_else(|| fs::read_to_string(&path))?;
                    let line = params["position"]["line"].as_u64().unwrap_or(0) as usize;
                    let column = params["position"]["character"].as_u64().unwrap_or(0) as usize;
                    let line_text = source.lines().nth(line).unwrap_or("");
                    let mut utf16 = 0;
                    let mut char_column = 0;
                    for c in line_text.chars() {
                        if utf16 >= column {
                            break;
                        }
                        utf16 += c.len_utf16();
                        char_column += 1;
                    }
                    let token = lex(&source)?.into_iter().find(|t| {
                        t.line == line + 1
                            && t.col <= char_column + 1
                            && t.col + t.text.chars().count() > char_column + 1
                    });
                    let Some(token) = token else {
                        return Ok(Json::Null);
                    };
                    let name = resolve_alias(&p, &token.text);
                    let Some(f) = p.functions.get(&name) else {
                        let mut checker = Checker {
                            program: &p,
                            scopes: vec![BTreeMap::new()],
                            return_ty: None,
                            loop_depth: 0,
                            bounds: BTreeMap::new(),
                            origin: path.clone(),
                        };
                        for s in &p.stmts {
                            if s.at.line <= token.line {
                                let _ = checker.stmt(s);
                            }
                        }
                        for f in p
                            .functions
                            .values()
                            .filter(|f| f.origin == path && f.at.line <= token.line)
                        {
                            let mut end = f.at.line;
                            expressions(&f.body, &mut |e| end = end.max(e.at.line));
                            if token.line <= end {
                                checker.bounds = f
                                    .type_params
                                    .iter()
                                    .filter_map(|(n, b)| b.as_ref().map(|b| (n.clone(), b.clone())))
                                    .collect();
                                for (n, t) in &f.params {
                                    checker.scopes[0].insert(n.clone(), (t.clone(), false));
                                }
                                for s in &f.body {
                                    if s.at.line <= token.line {
                                        let _ = checker.stmt(s);
                                    }
                                }
                                break;
                            }
                        }
                        if method.ends_with("hover") {
                            if let Some((ty, _)) = checker.find(&token.text) {
                                return Ok(
                                    json!({"contents":{"kind":"markdown","value":format!("`{}: {ty}`",token.text)}}),
                                );
                            }
                        }
                        return Ok(Json::Null);
                    };
                    if method.ends_with("definition") {
                        let source = documents
                            .get(&f.origin)
                            .cloned()
                            .map(Ok)
                            .unwrap_or_else(|| fs::read_to_string(&f.origin))?;
                        let column = utf16_column(
                            &source,
                            f.at.line.saturating_sub(1),
                            f.at.col.saturating_sub(1),
                        );
                        return Ok(
                            json!({"uri":file_uri(&f.origin),"range":{"start":{"line":f.at.line.saturating_sub(1),"character":column},"end":{"line":f.at.line.saturating_sub(1),"character":column+1}}}),
                        );
                    }
                    Ok(
                        json!({"contents":{"kind":"markdown","value":format!("`{}fn {}({}) -> {} effects {:?}`",if f.asynchronous {"async "}else{""},token.text,f.params.iter().map(|(n,t)|format!("{n}: {t}")).collect::<Vec<_>>().join(", "),f.ret,f.effects)}}),
                    )
                }
                "initialized" => Ok(Json::Null),
                _ => Err(Error::InvalidOperation(format!(
                    "unsupported LSP method {method}"
                ))),
            }
        })();
        if method == "exit" {
            return if shutdown {
                Ok(())
            } else {
                Err(Error::InvalidOperation("LSP exit before shutdown".into()))
            };
        }
        if let Some(id) = id {
            let value = match response {
                Ok(result) => json!({"jsonrpc":"2.0","id":id,"result":result}),
                Err(error) => {
                    json!({"jsonrpc":"2.0","id":id,"error":{"code":-32602,"message":error.to_string()}})
                }
            };
            send(&mut output, &value)?;
        }
    }
}
pub fn debug_session(path: &Path, root: &Path, options: RunOptions) -> Result<()> {
    if fs::metadata(path)?.len() > 128 * 1024 * 1024 {
        return Err(Error::InvalidOperation("debug trace exceeds budget".into()));
    }
    let trace: Json = serde_json::from_slice(&fs::read(path)?)
        .map_err(|e| Error::InvalidOperation(e.to_string()))?;
    if trace["format"] != 1 || trace["compiler"] != env!("CARGO_PKG_VERSION") {
        return Err(Error::InvalidOperation(
            "unsupported debug trace format/compiler; rebuild the recording with this compiler"
                .into(),
        ));
    }
    if let Some(key) = &options.verify_key {
        packages::verify_file(
            path,
            &PathBuf::from(format!("{}.signature", path.display())),
            key,
            "trace",
        )?;
    }
    let entry = trace["entry"]
        .as_str()
        .ok_or_else(|| Error::InvalidOperation("missing trace entry".into()))?;
    let entry = Path::new(entry);
    if entry.is_absolute()
        || entry
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err(Error::InvalidPath("debug entry".into()));
    }
    let source = root.join(entry);
    let parsed = if source.exists() {
        if let Some(c) = project::ProjectConfig::load(root)? {
            c.lock(root, false)?;
        }
        Some(program(root, &source, &BTreeMap::new())?)
    } else {
        None
    };
    let events = trace["events"]
        .as_array()
        .ok_or_else(|| Error::InvalidOperation("missing events".into()))?;
    let checkpoints = trace["debug"]["checkpoints"]
        .as_array()
        .ok_or_else(|| Error::InvalidOperation("missing checkpoints".into()))?;
    let mut index = 0usize;
    let mut checkpoint: Option<usize> = None;
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut current = if let Some(p) = &parsed {
        vm::debug_view(p.clone(), root, &trace, 0, options.clone())?
    } else {
        trace["debug"]["final"].clone()
    };
    let mut output = stdout.lock();
    writeln!(
        output,
        "REWIND recorded debugger: step, continue, checkpoint NAME, state, tasks, files, quit"
    )?;
    output.flush()?;
    loop {
        let mut command = String::new();
        if stdin.lock().read_line(&mut command)? == 0 {
            break;
        }
        let command = command.trim();
        let value = match command {
            "step" => {
                let event = events.get(index).cloned().unwrap_or(Json::Null);
                index = index.saturating_add(1).min(events.len());
                checkpoint = None;
                if let Some(p) = &parsed {
                    current = vm::debug_view(p.clone(), root, &trace, index, options.clone())?;
                }
                json!({"event":index,"instruction":event,"state_available":parsed.is_some()})
            }
            "continue" => {
                index = events.len();
                checkpoint = None;
                if let Some(p) = &parsed {
                    current = vm::debug_view(p.clone(), root, &trace, index, options.clone())?;
                }
                trace["result"].clone()
            }
            "state" => checkpoint
                .map(|i| &checkpoints[i])
                .unwrap_or(&current)
                .clone(),
            "tasks" => checkpoint
                .map(|i| &checkpoints[i]["scheduler"])
                .unwrap_or(&current["scheduler"])
                .clone(),
            "files" => checkpoint
                .map(|i| &checkpoints[i]["runtime"]["file_deltas"])
                .unwrap_or(&current["runtime"]["file_deltas"])
                .clone(),
            "quit" => break,
            _ => {
                if let Some(name) = command.strip_prefix("checkpoint ") {
                    checkpoint = checkpoints.iter().rposition(|c| c["checkpoint"] == name);
                    if let Some(i) = checkpoint {
                        index = checkpoints[i]["event_index"].as_u64().unwrap_or(0) as usize;
                    }
                    json!({"checkpoint":checkpoint.map(|i|&checkpoints[i]["checkpoint"]),"found":checkpoint.is_some()})
                } else {
                    json!({"error":"unknown debugger command"})
                }
            }
        };
        writeln!(output, "{}", value)?;
        output.flush()?;
    }
    Ok(())
}
