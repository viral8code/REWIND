use super::*;
mod symbols;
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
    let text = if text.starts_with('/') {
        text.strip_prefix('/').unwrap_or(&text).to_string()
    } else if text.starts_with("localhost/") {
        text.trim_start_matches("localhost/").to_string()
    } else {
        format!("//{text}")
    };
    Ok(PathBuf::from(text))
}
fn file_uri(path: &Path) -> String {
    let mut text = path.to_string_lossy().into_owned();
    #[cfg(windows)]
    {
        if let Some(unc) = text.strip_prefix(r"\\?\UNC\") {
            text = format!(r"\\{unc}");
        } else if let Some(local) = text.strip_prefix(r"\\?\") {
            text = local.into();
        }
    }
    text = text.replace('\\', "/");
    let mut encoded = String::new();
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"/-._~:".contains(&byte) {
            encoded.push(byte as char);
        } else {
            use std::fmt::Write;
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    if encoded.starts_with("//") {
        format!("file:{encoded}")
    } else {
        format!("file:///{}", encoded.trim_start_matches('/'))
    }
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
    let config = Some(match project::ProjectConfig::load(root)? {
        Some(config) => config,
        None => standalone_config(root, path.to_path_buf(), &BTreeSet::new())?,
    });
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
        {
            validate(&program, c)?;
        }}
    Ok(program)
}
fn workspace_program(
    root: &Path,
    path: &Path,
    documents: &BTreeMap<PathBuf, String>,
) -> Result<Program> {
    if let Some(config) = project::ProjectConfig::load(root)? {
        let p = program(root, &config.entry, documents)?;
        if p.included_modules.contains(path) {
            return Ok(p);
        }
    }
    program(root, path, documents)
}
fn send(writer: &mut impl Write, value: &Json) -> Result<()> {
    let bytes = serde_json::to_vec(value).map_err(|e| Error::InvalidOperation(e.to_string()))?;
    write!(writer, "Content-Length: {}\r\n\r\n", bytes.len())?;
    writer.write_all(&bytes)?;
    writer.flush()?;
    Ok(())
}
fn utf16_offset(text: &str, position: &Json) -> Result<usize> {
    let line = position["line"]
        .as_u64()
        .ok_or_else(|| Error::InvalidOperation("missing line".into()))? as usize;
    let column = position["character"]
        .as_u64()
        .ok_or_else(|| Error::InvalidOperation("missing character".into()))?
        as usize;
    let mut offset = 0;
    for (index, part) in text
        .split_inclusive('\n')
        .chain(std::iter::once(""))
        .enumerate()
    {
        if index == line {
            let content = part.trim_end_matches(['\r', '\n']);
            let mut units = 0;
            for (byte, c) in content.char_indices() {
                if units == column {
                    return Ok(offset + byte);
                }
                units += c.len_utf16();
                if units > column {
                    return Err(Error::InvalidOperation(
                        "UTF-16 position splits a surrogate pair".into(),
                    ));
                }
            }
            if units == column {
                return Ok(offset + content.len());
            }
            break;
        }
        offset += part.len();
    }
    Err(Error::InvalidOperation(
        "document position out of range".into(),
    ))
}
pub fn lsp(root: &Path) -> Result<()> {
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let stdout = io::stdout();
    let mut output = stdout.lock();
    let mut documents: BTreeMap<PathBuf, String> = BTreeMap::new();
    let mut versions: BTreeMap<PathBuf, i64> = BTreeMap::new();
    let modern = project::ProjectConfig::load(root)?.is_none_or(|_c| {
        true
    });
    let mut checked: BTreeMap<PathBuf, Program> = BTreeMap::new();
    let incremental = project::ProjectConfig::load(root)?.is_none_or(|_c| {
        true
    });
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
                    json!({"capabilities":{"textDocumentSync":if incremental {2}else{1},"hoverProvider":true,"documentFormattingProvider":true,"definitionProvider":true,"completionProvider":{"triggerCharacters":["."]},"referencesProvider":incremental,"renameProvider":if incremental {json!({"prepareProvider":true})} else {json!(false)},"signatureHelpProvider":if incremental {json!({"triggerCharacters":["(",","]})} else {Json::Null},"codeActionProvider":incremental,"documentSymbolProvider":true,"semanticTokensProvider":{"legend":{"tokenTypes":crate::v2::editor::SEMANTIC_LEGEND,"tokenModifiers":[]},"full":true},"experimental":{"rewindTimeline":modern,"provisionalDeclarations":modern}},"serverInfo":{"name":"REWIND","version":env!("CARGO_PKG_VERSION")}}),
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
                        checked.remove(&path);
                        versions.remove(&path);
                        send(
                            &mut output,
                            &json!({"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{"uri":uri,"diagnostics":[]}}),
                        )?;
                        return Ok(Json::Null);
                    }
                    let version = params["textDocument"]["version"].as_i64().ok_or_else(|| {
                        Error::InvalidOperation("document version required".into())
                    })?;
                    if versions.get(&path).is_some_and(|old| version <= *old) {
                        return Err(Error::InvalidOperation("stale document version".into()));
                    }
                    let text = if method.ends_with("didOpen") {
                        params["textDocument"]["text"]
                            .as_str()
                            .ok_or_else(|| {
                                Error::InvalidOperation("document text required".into())
                            })?
                            .to_string()
                    } else {
                        let mut text = documents.get(&path).cloned().ok_or_else(|| {
                            Error::InvalidOperation("document must be opened first".into())
                        })?;
                        let changes = params["contentChanges"].as_array().ok_or_else(|| {
                            Error::InvalidOperation("contentChanges required".into())
                        })?;
                        for change in changes {
                            let replacement = change["text"].as_str().ok_or_else(|| {
                                Error::InvalidOperation("change text required".into())
                            })?;
                            if let Some(range) = change.get("range") {
                                let start = utf16_offset(&text, &range["start"])?;
                                let end = utf16_offset(&text, &range["end"])?;
                                if start > end {
                                    return Err(Error::InvalidOperation(
                                        "reversed edit range".into(),
                                    ));
                                }
                                text.replace_range(start..end, replacement);
                            } else {
                                text = replacement.into();
                            }
                            if text.len() > 512 * 1024 {
                                return Err(Error::InvalidOperation(
                                    "LSP document budget exceeded".into(),
                                ));
                            }
                        }
                        text
                    };
                    if text.len() > 512 * 1024
                        || documents.len() >= 128 && !documents.contains_key(&path)
                    {
                        return Err(Error::InvalidOperation(
                            "LSP document budget exceeded".into(),
                        ));
                    }
                    documents.insert(path.clone(), text.clone());
                    versions.insert(path.clone(), version);
                    let diagnostics = match workspace_program(root, &path, &documents) {
                        Ok(p) => {
                            checked.insert(path.clone(), p);
                            Vec::new()
                        }
                        Err(e) => symbols::diagnostics(root, &path, &text, &e),
                    };
                    send(
                        &mut output,
                        &json!({"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{"uri":uri,"version":params["textDocument"]["version"],"diagnostics":diagnostics}}),
                    )?;
                    Ok(Json::Null)
                }
                "textDocument/references"
                | "textDocument/rename"
                | "textDocument/prepareRename"
                | "textDocument/signatureHelp" => {
                    if !incremental {
                        return Err(Error::InvalidOperation(
                            "LSP request requires language 0.6".into(),
                        ));
                    }
                    let path = document_path(params["textDocument"]["uri"].as_str().unwrap_or(""))?;
                    if !path.starts_with(fs::canonicalize(root)?) {
                        return Err(Error::InvalidPath(path.display().to_string()));
                    }
                    let loaded = workspace_program(root, &path, &documents);
                    let provisional = loaded.is_err();
                    let p = match loaded {
                        Ok(p) => p,
                        Err(_) if method.ends_with("signatureHelp") => {
                            if modern {
                                checked.get(&path).cloned().map(Ok).unwrap_or_else(|| {
                                    let mut saved = documents.clone();
                                    saved.remove(&path);
                                    workspace_program(root, &path, &saved)
                                })?
                            } else {
                                let mut saved = documents.clone();
                                saved.remove(&path);
                                workspace_program(root, &path, &saved)?
                            }
                        }
                        Err(e) => return Err(e),
                    };
                    let mut result = symbols::request(root, method, &p, &path, params, &documents)?;
                    if modern && method.ends_with("signatureHelp") && result.is_object() {
                        result["data"] = json!({"provisional":provisional});
                    }
                    Ok(result)
                }
                "textDocument/formatting" => {
                    let uri = params["textDocument"]["uri"]
                        .as_str()
                        .ok_or_else(|| Error::InvalidOperation("missing document URI".into()))?;
                    let path = document_path(uri)?;
                    if !path.starts_with(fs::canonicalize(root)?) {
                        return Err(Error::InvalidPath(uri.into()));
                    }
                    let text = documents
                        .get(&path)
                        .cloned()
                        .map(Ok)
                        .unwrap_or_else(|| fs::read_to_string(&path))?;
                    let formatted = format_source(&text)?;
                    if text == formatted {
                        return Ok(json!([]));
                    }
                    let line = text.chars().filter(|c| *c == '\n').count();
                    let column = text
                        .rsplit('\n')
                        .next()
                        .unwrap_or("")
                        .encode_utf16()
                        .count();
                    Ok(
                        json!([{"range":{"start":{"line":0,"character":0},"end":{"line":line,"character":column}},"newText":formatted}]),
                    )
                }
                "textDocument/documentSymbol" | "textDocument/semanticTokens/full" => {
                    let uri = params["textDocument"]["uri"]
                        .as_str()
                        .ok_or_else(|| Error::InvalidOperation("missing document URI".into()))?;
                    let path = document_path(uri)?;
                    if !path.starts_with(fs::canonicalize(root)?) {
                        return Err(Error::InvalidPath(uri.into()));
                    }
                    let text = documents
                        .get(&path)
                        .cloned()
                        .map(Ok)
                        .unwrap_or_else(|| fs::read_to_string(&path))?;
                    if method == "textDocument/semanticTokens/full" {
                        return Ok(json!({"data": crate::v2::editor::semantic_tokens(&text)}));
                    }
                    Ok(Json::Array(
                        crate::v2::editor::symbols(&text)
                            .iter()
                            .map(|s| {
                                let kind = match s["kind"].as_str().unwrap_or("") {
                                    "function" => 12,
                                    "method" => 6,
                                    "struct" => 23,
                                    "enum" => 10,
                                    "trait" => 11,
                                    "impl" => 5,
                                    "const" => 14,
                                    _ => 26,
                                };
                                let at = |line: &str, col: &str| {
                                    let l = s[line].as_u64().unwrap_or(1) as usize - 1;
                                    let c = s[col].as_u64().unwrap_or(1) as usize - 1;
                                    json!({"line": l, "character": utf16_column(&text, l, c)})
                                };
                                let range = json!({"start": at("line", "column"), "end": at("endLine", "endColumn")});
                                json!({"name": s["name"], "kind": kind, "range": range, "selectionRange": range})
                            })
                            .collect(),
                    ))
                }
                "textDocument/codeAction" => {
                    if !incremental {
                        return Err(Error::InvalidOperation(
                            "LSP request requires language 0.6".into(),
                        ));
                    }
                    symbols::quick_fixes(root, params, &documents)
                }
                "textDocument/hover" | "textDocument/definition" | "textDocument/completion" => {
                    let path = document_path(params["textDocument"]["uri"].as_str().unwrap_or(""))?;
                    if !path.starts_with(fs::canonicalize(root)?) {
                        return Err(Error::InvalidPath(path.display().to_string()));
                    }
                    let loaded = if incremental {
                        workspace_program(root, &path, &documents)
                    } else {
                        program(root, &path, &documents)
                    };
                    let provisional = loaded.is_err();
                    let p = match loaded {
                        Ok(p) => {
                            checked.insert(path.clone(), p.clone());
                            p
                        }
                        Err(_) if modern && !method.ends_with("definition") => {
                            checked.get(&path).cloned().map(Ok).unwrap_or_else(|| {
                                let mut saved = documents.clone();
                                saved.remove(&path);
                                workspace_program(root, &path, &saved)
                            })?
                        }
                        Err(e) => return Err(e),
                    };
                    if incremental && method.ends_with("definition") {
                        return symbols::request(root, method, &p, &path, params, &documents);
                    }
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
                                .map(|n| json!({"label":n,"kind":3,"data":{"provisional":provisional},"detail":if provisional {"last checked declaration"}else{"checked declaration"}}))
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
                    let token = editor_tokens(&source, modern)?.into_iter().find(|t| {
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
                            scopes: vec![crate::v2::Scope::default()],
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
                                    json!({"contents":{"kind":"markdown","value":format!("`{}: {ty}`{}",token.text,if provisional {" (last checked declaration)"}else{""})}}),
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
                        json!({"contents":{"kind":"markdown","value":format!("`{}fn {}({}) -> {} effects {:?}`{}",if f.asynchronous {"async "}else{""},token.text,f.params.iter().map(|(n,t)|format!("{n}: {t}")).collect::<Vec<_>>().join(", "),f.ret,f.effects,if provisional {" (last checked declaration)"}else{""})}}),
                    )
                }
                "rewind/timeline" if modern => {
                    let path = document_path(params["uri"].as_str().unwrap_or(""))?;
                    if !path.starts_with(fs::canonicalize(root)?) {
                        return Err(Error::InvalidPath(path.display().to_string()));
                    }
                    if fs::metadata(&path)?.len() > 128 * 1024 * 1024 {
                        return Err(Error::InvalidOperation(
                            "timeline trace exceeds budget".into(),
                        ));
                    }
                    if let Some(key) = params["publicKey"].as_str() {
                        let trace: Json = serde_json::from_slice(&fs::read(&path)?)
                            .map_err(|e| Error::InvalidOperation(e.to_string()))?;
                        packages::verify_file(
                            &path,
                            &PathBuf::from(format!("{}.signature", path.display())),
                            key,
                            if trace["kind"] == "rewind-inspection" {
                                "inspection"
                            } else {
                                "trace"
                            },
                        )?;
                    }
                    v07::timeline_data(
                        &path,
                        params["from"].as_u64().unwrap_or(0) as usize,
                        params["count"].as_u64().unwrap_or(100) as usize,
                        params["task"].as_u64(),
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
fn debugger_view(
    parsed: Option<&Program>,
    root: &Path,
    trace: &Json,
    index: usize,
    options: &RunOptions,
) -> Result<Json> {
    if let Some(view) = v06::debug::view(trace, index)? {
        return Ok(view);
    }
    if let Some(p) = parsed {
        vm::debug_view(p.clone(), root, trace, index, options.clone())
    } else {
        Ok(trace["debug"]["final"].clone())
    }
}
/// A recorded-execution debugger over a validated trace. The command-line session and
/// the Debug Adapter Protocol server both drive this engine. It restores state from the
/// recorded index (or by replaying the saved source) and never re-runs the program's
/// external operations.
pub struct DebugEngine {
    trace: Json,
    root: PathBuf,
    options: RunOptions,
    parsed: Option<Program>,
    indexed: bool,
    index: usize,
    selected_task: Option<u64>,
    breakpoints: BTreeSet<(String, usize)>,
    current: Json,
}
impl DebugEngine {
    /// Reads and validates a debug trace: size, signature, format, fingerprint and
    /// index length. Compact recordings have no step history and are refused.
    pub fn open(path: &Path, root: &Path, options: RunOptions) -> Result<Self> {
        if fs::metadata(path)?.len() > 128 * 1024 * 1024 {
            return Err(Error::InvalidOperation("debug trace exceeds budget".into()));
        }
        let trace: Json = serde_json::from_slice(&fs::read(path)?)
            .map_err(|e| Error::InvalidOperation(e.to_string()))?;
        let signature_kind = if trace["kind"] == "rewind-inspection" {
            "inspection"
        } else {
            "trace"
        };
        let trace = v08::inspection_trace(trace)?;
        if trace["record_mode"] == "compact" {
            return Err(Error::InvalidOperation(
                "CompactTraceNoDebugHistory: record with --record-mode debug for stepping".into(),
            ));
        }
        if !v08::readable_trace(&trace) {
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
                signature_kind,
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
        let events = trace["events"]
            .as_array()
            .ok_or_else(|| Error::InvalidOperation("missing events".into()))?;
        let indexed = !trace["debug"]["index"].is_null();
        if indexed
            && trace["debug"]["index"]["deltas"]
                .as_array()
                .is_none_or(|a| a.len() != events.len())
        {
            return Err(Error::InvalidOperation("invalid debug index length".into()));
        }
        if indexed && trace["artifact_sha256"] != trace["fingerprint"] {
            return Err(Error::InvalidOperation(
                "debug artifact fingerprint mismatch".into(),
            ));
        }
        let source = root.join(entry);
        let parsed = if !indexed && source.exists() {
            if let Some(c) = project::ProjectConfig::load(root)? {
                c.lock(root, false)?;
            }
            Some(program(root, &source, &BTreeMap::new())?)
        } else {
            None
        };
        if trace["debug"]["checkpoints"].as_array().is_none() {
            return Err(Error::InvalidOperation("missing checkpoints".into()));
        }
        let current = debugger_view(parsed.as_ref(), root, &trace, 0, &options)?;
        Ok(Self {
            trace,
            root: root.to_path_buf(),
            options,
            parsed,
            indexed,
            index: 0,
            selected_task: None,
            breakpoints: BTreeSet::new(),
            current,
        })
    }
    fn events(&self) -> &Vec<Json> {
        self.trace["events"].as_array().expect("validated in open")
    }
    pub fn event_count(&self) -> usize {
        self.events().len()
    }
    pub fn index(&self) -> usize {
        self.index
    }
    pub fn current(&self) -> &Json {
        &self.current
    }
    pub fn state_available(&self) -> bool {
        self.indexed || self.parsed.is_some()
    }
    /// The recorded event nearest before `index`, if any.
    pub fn event_at(&self, index: usize) -> Option<&Json> {
        self.events().get(index)
    }
    fn matches_task(&self, event: usize) -> bool {
        self.selected_task
            .is_none_or(|task| self.events()[event]["task"] == task)
    }
    fn go(&mut self, index: usize) -> Result<()> {
        if index != self.index {
            self.current = debugger_view(
                self.parsed.as_ref(),
                &self.root,
                &self.trace,
                index,
                &self.options,
            )?;
            self.index = index;
        }
        Ok(())
    }
    pub fn step(&mut self) -> Result<Json> {
        let end = self.event_count();
        let index = (self.index..end)
            .find(|i| self.matches_task(*i))
            .map(|i| i + 1)
            .unwrap_or(end);
        self.go(index)?;
        Ok(
            json!({"event":index,"instruction":self.events().get(index.saturating_sub(1)),"state_available":self.state_available()}),
        )
    }
    pub fn back(&mut self) -> Result<Json> {
        let index = (0..self.index)
            .rev()
            .find(|i| self.matches_task(*i))
            .unwrap_or(0);
        self.go(index)?;
        Ok(
            json!({"event":index,"instruction":self.events().get(index),"state_available":self.state_available()}),
        )
    }
    pub fn move_line(&mut self, forward: bool) -> Result<Json> {
        let end = self.event_count();
        let base = self.events().get(self.index.min(end.saturating_sub(1)));
        let range: Box<dyn Iterator<Item = usize>> = if forward {
            Box::new(self.index + 1..end)
        } else {
            Box::new((0..self.index).rev())
        };
        let index = range
            .filter(|i| self.matches_task(*i))
            .find(|i| {
                base.is_none_or(|base| {
                    self.events()[*i]["source"] != base["source"]
                        || self.events()[*i]["line"] != base["line"]
                })
            })
            .unwrap_or(if forward { end } else { 0 });
        self.go(index)?;
        Ok(json!({"event":index,"instruction":self.events().get(index)}))
    }
    pub fn run_to_breakpoint(&mut self, forward: bool) -> Result<Json> {
        let end = self.event_count();
        let range: Box<dyn Iterator<Item = usize>> = if forward {
            Box::new(self.index + 1..end)
        } else {
            Box::new((0..self.index).rev())
        };
        let stop = range.filter(|i| self.matches_task(*i)).find(|i| {
            self.breakpoints.contains(&(
                self.events()[*i]["source"].as_str().unwrap_or("").into(),
                self.events()[*i]["line"].as_u64().unwrap_or(0) as usize,
            ))
        });
        let index = stop.unwrap_or(if forward { end } else { 0 });
        self.go(index)?;
        Ok(
            json!({"event":index,"breakpoint":stop.is_some(),"result":if index==end{self.trace["result"].clone()}else{Json::Null}}),
        )
    }
    pub fn state(&self) -> Json {
        let current = &self.current;
        if let Some(task) = self.selected_task {
            json!({"event":self.index,"task":task,"task_state":current["scheduler"]["tasks"][task.to_string()],"frames":if current["scheduler"]["active"]==task{current["frames"].clone()}else{current["task_frames"][task.to_string()].clone()},"runtime":current["runtime"]})
        } else {
            current.clone()
        }
    }
    pub fn tasks(&self) -> Json {
        self.current["scheduler"].clone()
    }
    pub fn files(&self) -> Json {
        self.current["runtime"]["file_deltas"].clone()
    }
    pub fn go_to_checkpoint(&mut self, name: &str) -> Result<Json> {
        let target = self.trace["debug"]["checkpoints"]
            .as_array()
            .and_then(|all| all.iter().rfind(|c| c["checkpoint"] == name))
            .map(|c| c["event_index"].as_u64().unwrap_or(0) as usize);
        if let Some(index) = target {
            self.go(index.min(self.event_count()))?;
        }
        Ok(json!({"checkpoint":name,"found":target.is_some(),"event":self.index}))
    }
    pub fn select_task(&mut self, task: u64) -> Json {
        if self.current["scheduler"]["tasks"]
            .get(task.to_string())
            .is_some()
        {
            self.selected_task = Some(task);
            json!({"task":task})
        } else {
            json!({"error":"unknown task"})
        }
    }
    pub fn set_breakpoint(&mut self, source: &str, line: usize, enabled: bool) -> Json {
        if enabled {
            self.breakpoints.insert((source.to_string(), line));
        } else {
            self.breakpoints.remove(&(source.to_string(), line));
        }
        json!({"source":source,"line":line,"enabled":enabled})
    }
    /// Replaces every breakpoint of one source file at once (DAP setBreakpoints).
    pub fn replace_breakpoints(&mut self, source: &str, lines: &[usize]) {
        self.breakpoints.retain(|(s, _)| s != source);
        for line in lines {
            self.breakpoints.insert((source.to_string(), *line));
        }
    }
    pub fn diff(&self, other: usize) -> Result<Json> {
        if other > self.event_count() {
            return Ok(json!({"error":"event out of range"}));
        }
        let before = debugger_view(
            self.parsed.as_ref(),
            &self.root,
            &self.trace,
            other,
            &self.options,
        )?;
        Ok(
            json!({"from":other,"to":self.index,"files":v06::debug::delta(&before["runtime"]["file_deltas"],&self.current["runtime"]["file_deltas"])}),
        )
    }
    /// Runs one text command of the interactive debugger. Returns the JSON reply and
    /// whether the session should end.
    pub fn command(&mut self, command: &str) -> Result<(Json, bool)> {
        let value = match command {
            "step" => self.step()?,
            "back" | "reverse-step" => self.back()?,
            "next-line" => self.move_line(true)?,
            "previous-line" => self.move_line(false)?,
            "continue" => self.run_to_breakpoint(true)?,
            "reverse-continue" => self.run_to_breakpoint(false)?,
            "state" => self.state(),
            "tasks" => self.tasks(),
            "files" => self.files(),
            "quit" => return Ok((Json::Null, true)),
            _ => {
                if let Some(name) = command.strip_prefix("checkpoint ") {
                    self.go_to_checkpoint(name)?
                } else if let Some(text) = command.strip_prefix("task ") {
                    match text.parse::<u64>() {
                        Ok(task) => self.select_task(task),
                        _ => json!({"error":"unknown task"}),
                    }
                } else if let Some(text) = command
                    .strip_prefix("break ")
                    .or_else(|| command.strip_prefix("clear "))
                {
                    if let Some((source, line)) = text
                        .rsplit_once(':')
                        .and_then(|(s, n)| n.parse::<usize>().ok().map(|n| (s.to_string(), n)))
                    {
                        self.set_breakpoint(&source, line, !command.starts_with("clear "))
                    } else {
                        json!({"error":"expected SOURCE:LINE"})
                    }
                } else if let Some(text) = command.strip_prefix("diff ") {
                    match text.parse::<usize>() {
                        Ok(other) => self.diff(other)?,
                        Err(_) => json!({"error":"expected event number"}),
                    }
                } else {
                    json!({"error":"unknown debugger command"})
                }
            }
        };
        Ok((value, false))
    }
}
pub fn debug_session(path: &Path, root: &Path, options: RunOptions) -> Result<()> {
    let mut engine = DebugEngine::open(path, root, options)?;
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut output = stdout.lock();
    writeln!(output,"REWIND recorded debugger: step, back, next-line, previous-line, continue, reverse-continue, break SOURCE:LINE, clear SOURCE:LINE, task ID, checkpoint NAME, state, tasks, files, diff EVENT, quit")?;
    output.flush()?;
    loop {
        let mut command = String::new();
        if stdin.lock().read_line(&mut command)? == 0 {
            break;
        }
        let (value, quit) = engine.command(command.trim())?;
        if quit {
            break;
        }
        writeln!(output, "{value}")?;
        output.flush()?;
    }
    Ok(())
}

fn editor_tokens(source: &str, modern: bool) -> Result<Vec<Tok>> {
    lex(source).or_else(|e| {
        if modern {
            lex(&format!("{source}\"")).or_else(|_| lex(&format!("{source}*/")))
        } else {
            Err(e)
        }
    })
}
