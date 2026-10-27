//! Debug Adapter Protocol over stdio for recorded executions (DBG-03).
//!
//! The adapter opens a validated debug recording and moves through it with the
//! `DebugEngine`. It never starts the program, never evaluates arbitrary expressions
//! and never listens on a socket. One process serves one client.
use super::debug_values::{Browser, Var};
use super::tooling::DebugEngine;
use super::*;
use serde_json::{json, Value as Json};
use std::io::{BufRead, Read, Write};

const MAX_MESSAGE_BYTES: usize = 1024 * 1024;
const MAX_HEADER_BYTES: usize = 1024;
const MAX_JSON_DEPTH: usize = 64;
const MAX_STACK_FRAMES: usize = 256;

fn json_depth_ok(bytes: &[u8]) -> bool {
    let (mut depth, mut quoted, mut escaped) = (0usize, false, false);
    for &b in bytes {
        if quoted {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                quoted = false;
            }
            continue;
        }
        match b {
            b'"' => quoted = true,
            b'{' | b'[' => {
                depth += 1;
                if depth > MAX_JSON_DEPTH {
                    return false;
                }
            }
            b'}' | b']' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    true
}

/// Reads one framed message. `Ok(None)` is a clean end of input.
fn read_message(input: &mut impl BufRead) -> std::result::Result<Option<Json>, String> {
    let mut length = None;
    let mut saw_header = false;
    loop {
        let mut line = Vec::new();
        let read = (&mut *input)
            .take(MAX_HEADER_BYTES as u64 + 1)
            .read_until(b'\n', &mut line)
            .map_err(|e| e.to_string())?;
        if read == 0 {
            return if saw_header {
                Err("input ended inside a message header".into())
            } else {
                Ok(None)
            };
        }
        if line.len() > MAX_HEADER_BYTES {
            return Err("message header is too long".into());
        }
        saw_header = true;
        let text = String::from_utf8_lossy(&line);
        let text = text.trim_end_matches(['\r', '\n']);
        if text.is_empty() {
            break;
        }
        if let Some(value) = text
            .strip_prefix("Content-Length:")
            .or_else(|| text.strip_prefix("content-length:"))
        {
            length = Some(
                value
                    .trim()
                    .parse::<usize>()
                    .map_err(|_| "invalid Content-Length".to_string())?,
            );
        }
    }
    let length = length.ok_or("missing Content-Length")?;
    if length > MAX_MESSAGE_BYTES {
        return Err(format!(
            "message of {length} bytes exceeds the {MAX_MESSAGE_BYTES} byte limit"
        ));
    }
    let mut body = vec![0u8; length];
    input
        .read_exact(&mut body)
        .map_err(|_| "input ended inside a message body".to_string())?;
    if !json_depth_ok(&body) {
        return Err("message nests deeper than the limit".into());
    }
    serde_json::from_slice(&body)
        .map(Some)
        .map_err(|e| format!("invalid JSON: {e}"))
}

struct Session<W: Write> {
    out: W,
    seq: u64,
    root: PathBuf,
    options: RunOptions,
    engine: Option<DebugEngine>,
    browser: Browser,
}

impl<W: Write> Session<W> {
    fn send(&mut self, mut message: Json) -> Result<()> {
        self.seq += 1;
        message["seq"] = json!(self.seq);
        let body = message.to_string();
        write!(self.out, "Content-Length: {}\r\n\r\n{body}", body.len())?;
        self.out.flush()?;
        Ok(())
    }
    fn respond(
        &mut self,
        request: &Json,
        success: bool,
        body: Json,
        message: Option<&str>,
    ) -> Result<()> {
        let mut response = json!({
            "type":"response",
            "request_seq":request["seq"],
            "success":success,
            "command":request["command"],
            "body":body,
        });
        if let Some(message) = message {
            response["message"] = json!(message);
        }
        self.send(response)
    }
    fn fail(&mut self, request: &Json, message: &str) -> Result<()> {
        self.respond(
            request,
            false,
            json!({"error":{"id":1,"format":message}}),
            Some(message),
        )
    }
    fn event(&mut self, name: &str, body: Json) -> Result<()> {
        self.send(json!({"type":"event","event":name,"body":body}))
    }
    fn stopped(&mut self, reason: &str, thread: u64) -> Result<()> {
        self.event(
            "stopped",
            json!({"reason":reason,"threadId":thread,"allThreadsStopped":true}),
        )
    }
    fn engine(&mut self) -> std::result::Result<&mut DebugEngine, String> {
        self.engine
            .as_mut()
            .ok_or_else(|| "no recording is open: send launch first".to_string())
    }
    fn frames_of(engine: &DebugEngine, thread: u64) -> Vec<Json> {
        let view = engine.current();
        let list = if view["scheduler"]["active"] == thread {
            &view["frames"]
        } else {
            &view["task_frames"][thread.to_string()]
        };
        list.as_array().cloned().unwrap_or_default()
    }
    fn location(
        engine: &DebugEngine,
        thread: u64,
        frames: &[Json],
        from_inner: usize,
    ) -> (String, u64, u64) {
        // The innermost frame is at the event where the debugger stands; callers are
        // at the Call instruction that created the next frame.
        let end = engine.event_count();
        let mut position = engine.index().min(end.saturating_sub(1));
        if from_inner > 0 {
            let child = &frames[frames.len() - from_inner];
            let wanted = child["return_pc"].as_u64().unwrap_or(0).saturating_sub(1);
            if let Some(found) = (0..=position).rev().find(|i| {
                engine
                    .event_at(*i)
                    .is_some_and(|e| e["task"] == thread && e["pc"].as_u64() == Some(wanted))
            }) {
                position = found;
            }
        }
        match engine.event_at(position) {
            Some(e) => (
                e["source"].as_str().unwrap_or("").to_string(),
                e["line"].as_u64().unwrap_or(0),
                e["column"].as_u64().unwrap_or(0),
            ),
            None => (String::new(), 0, 0),
        }
    }
    fn vars_json(vars: Vec<Var>) -> Json {
        Json::Array(
            vars.into_iter()
                .map(|v| {
                    json!({"name":v.name,"value":v.value,"type":v.ty,"variablesReference":v.reference,
                    "indexedVariables":0,"namedVariables":v.total})
                })
                .collect(),
        )
    }
    fn frame_scopes(engine: &DebugEngine, thread: u64, frame: usize) -> Vec<Vec<(String, String)>> {
        let frames = Self::frames_of(engine, thread);
        // DAP frame 0 is the innermost; the recording lists the outermost first.
        let Some(frame) = frames
            .len()
            .checked_sub(1 + frame)
            .and_then(|i| frames.get(i))
        else {
            return Vec::new();
        };
        frame["scopes"]
            .as_array()
            .map(|scopes| {
                scopes
                    .iter()
                    .map(|scope| {
                        scope
                            .as_object()
                            .map(|m| {
                                m.iter()
                                    .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                                    .collect()
                            })
                            .unwrap_or_default()
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
    fn after_move(&mut self, reason: &str, thread: u64, forward: bool) -> Result<()> {
        self.browser.reset();
        let (at_end, at_start) = {
            let engine = self.engine.as_ref().expect("checked by caller");
            (engine.index() >= engine.event_count(), engine.index() == 0)
        };
        if forward && at_end {
            self.event("terminated", json!({}))
        } else {
            self.stopped(
                if !forward && at_start {
                    "entry"
                } else {
                    reason
                },
                thread,
            )
        }
    }

    fn handle(&mut self, request: &Json) -> Result<bool> {
        let command = request["command"].as_str().unwrap_or("").to_string();
        let args = request["arguments"].clone();
        match command.as_str() {
            "initialize" => {
                self.respond(
                    request,
                    true,
                    json!({
                        "supportsConfigurationDoneRequest":true,
                        "supportsStepBack":true,
                        "supportsEvaluateForHovers":true,
                        "supportsSetVariable":false,
                        "supportsRestartRequest":false,
                        "supportsTerminateRequest":false,
                        "supportsStepInTargetsRequest":false,
                        "supportsDataBreakpoints":false,
                        "supportsConditionalBreakpoints":false,
                        "supportsFunctionBreakpoints":false,
                        "supportsReadMemoryRequest":false,
                    }),
                    None,
                )?;
                self.event("initialized", json!({}))?;
            }
            "launch" => {
                let Some(trace) = args["trace"].as_str() else {
                    return self.fail(request, "launch needs arguments.trace: the path of a debug recording (rewind run --record trace.json --record-mode debug)").map(|_| false);
                };
                let mut options = self.options.clone();
                if let Some(key) = args["verifyKey"].as_str() {
                    options.verify_key = Some(key.to_string());
                }
                let root = args["root"]
                    .as_str()
                    .map(PathBuf::from)
                    .unwrap_or_else(|| self.root.clone());
                match DebugEngine::open(Path::new(trace), &root, options) {
                    Ok(engine) => {
                        self.engine = Some(engine);
                        self.browser.reset();
                        self.respond(request, true, json!({}), None)?;
                    }
                    Err(error) => {
                        let text = error.to_string();
                        self.fail(request, &text)?;
                    }
                }
            }
            "setBreakpoints" => {
                let path = args["source"]["path"]
                    .as_str()
                    .or(args["source"]["name"].as_str())
                    .unwrap_or("")
                    .to_string();
                let lines: Vec<usize> = args["breakpoints"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|b| b["line"].as_u64().map(|n| n as usize))
                            .collect()
                    })
                    .unwrap_or_default();
                let engine = match self.engine() {
                    Ok(engine) => engine,
                    Err(m) => return self.fail(request, &m).map(|_| false),
                };
                // Recorded events name sources relative to the project; match by suffix.
                let known: Vec<(String, usize)> = (0..engine.event_count())
                    .filter_map(|i| engine.event_at(i))
                    .filter_map(|e| {
                        Some((
                            e["source"].as_str()?.to_string(),
                            e["line"].as_u64()? as usize,
                        ))
                    })
                    .collect();
                let source = known
                    .iter()
                    .map(|(s, _)| s.clone())
                    .find(|s| path.ends_with(s.as_str()) || s.ends_with(&path))
                    .unwrap_or(path.clone());
                let verified: Vec<(usize, bool)> = lines
                    .iter()
                    .map(|line| (*line, known.iter().any(|(s, l)| *s == source && l == line)))
                    .collect();
                let accepted: Vec<usize> = verified
                    .iter()
                    .filter(|(_, ok)| *ok)
                    .map(|(l, _)| *l)
                    .collect();
                engine.replace_breakpoints(&source, &accepted);
                let body = verified
                    .into_iter()
                    .map(|(line, ok)| {
                        if ok {
                            json!({"verified":true,"line":line})
                        } else {
                            json!({"verified":false,"line":line,"message":"no recorded instruction on this line"})
                        }
                    })
                    .collect::<Vec<_>>();
                self.respond(request, true, json!({"breakpoints":body}), None)?;
            }
            "configurationDone" => {
                self.respond(request, true, json!({}), None)?;
                if self.engine.is_some() {
                    self.stopped("entry", 0)?;
                }
            }
            "threads" => {
                let engine = match self.engine() {
                    Ok(e) => e,
                    Err(m) => return self.fail(request, &m).map(|_| false),
                };
                let mut ids: Vec<u64> = engine.current()["scheduler"]["tasks"]
                    .as_object()
                    .map(|m| m.keys().filter_map(|k| k.parse().ok()).collect())
                    .unwrap_or_default();
                if !ids.contains(&0) {
                    ids.push(0);
                }
                ids.sort();
                let threads = ids
                    .into_iter()
                    .map(|id| json!({"id":id,"name":if id==0{"main task".to_string()}else{format!("task {id}")}}))
                    .collect::<Vec<_>>();
                self.respond(request, true, json!({"threads":threads}), None)?;
            }
            "stackTrace" => {
                let thread = args["threadId"].as_u64().unwrap_or(0);
                let generation_base = self.browser.generation() * 1000;
                let engine = match self.engine() {
                    Ok(e) => e,
                    Err(m) => return self.fail(request, &m).map(|_| false),
                };
                let frames = Self::frames_of(engine, thread);
                let mut out = Vec::new();
                for (depth, frame) in frames.iter().rev().take(MAX_STACK_FRAMES).enumerate() {
                    let (source, line, column) = Self::location(engine, thread, &frames, depth);
                    out.push(json!({
                        "id":generation_base + depth as u64,
                        "name":frame["function"],
                        "source":{"name":source,"path":source},
                        "line":line,
                        "column":column,
                    }));
                }
                if out.is_empty() {
                    let (source, line, column) = Self::location(engine, thread, &frames, 0);
                    out.push(json!({"id":generation_base,"name":"<entry>","source":{"name":source,"path":source},"line":line,"column":column}));
                }
                let total = frames.len().max(out.len());
                self.respond(
                    request,
                    true,
                    json!({"stackFrames":out,"totalFrames":total}),
                    None,
                )?;
            }
            "scopes" => {
                let frame_id = args["frameId"].as_u64().unwrap_or(u64::MAX);
                if frame_id / 1000 != self.browser.generation() {
                    return self
                        .fail(
                            request,
                            "stale frameId: the debugger moved to another event",
                        )
                        .map(|_| false);
                }
                let depth = (frame_id % 1000) as usize;
                let thread = args["threadId"].as_u64().unwrap_or(0);
                let scopes = match self.engine() {
                    Ok(engine) => Self::frame_scopes(engine, thread, depth),
                    Err(m) => return self.fail(request, &m).map(|_| false),
                };
                let mut out = Vec::new();
                for (index, entries) in scopes.into_iter().enumerate().rev() {
                    let (reference, count) = self.browser.scope(entries);
                    out.push(json!({"name":if index==0{"Locals".to_string()}else{format!("Scope {index}")},
                        "variablesReference":reference,"namedVariables":count,"expensive":false}));
                }
                self.respond(request, true, json!({"scopes":out}), None)?;
            }
            "variables" => {
                let reference = args["variablesReference"].as_u64().unwrap_or(0);
                let start = args["start"].as_u64().unwrap_or(0) as usize;
                let count = args["count"].as_u64().unwrap_or(100) as usize;
                let heap = match self.engine() {
                    Ok(e) => e.current()["runtime"]["heap"].clone(),
                    Err(m) => return self.fail(request, &m).map(|_| false),
                };
                match self.browser.variables(reference, start, count, &heap) {
                    Ok(vars) => self.respond(
                        request,
                        true,
                        json!({"variables":Self::vars_json(vars)}),
                        None,
                    )?,
                    Err(m) => self.fail(request, &m)?,
                }
            }
            "evaluate" => {
                let expression = args["expression"].as_str().unwrap_or("").to_string();
                let thread = args["threadId"].as_u64().unwrap_or(0);
                let depth = args["frameId"]
                    .as_u64()
                    .map(|f| (f % 1000) as usize)
                    .unwrap_or(0);
                if let Some(frame) = args["frameId"].as_u64() {
                    if frame / 1000 != self.browser.generation() {
                        return self
                            .fail(
                                request,
                                "stale frameId: the debugger moved to another event",
                            )
                            .map(|_| false);
                    }
                }
                let (scopes, heap) = match self.engine() {
                    Ok(e) => (
                        Self::frame_scopes(e, thread, depth),
                        e.current()["runtime"]["heap"].clone(),
                    ),
                    Err(m) => return self.fail(request, &m).map(|_| false),
                };
                match self.browser.watch(&expression, &scopes, &heap) {
                    Ok(var) => self.respond(
                        request,
                        true,
                        json!({"result":var.value,"type":var.ty,"variablesReference":var.reference,"namedVariables":var.total}),
                        None,
                    )?,
                    Err(m) => self.fail(request, &m)?,
                }
            }
            "next" | "stepIn" | "stepBack" | "continue" | "reverseContinue" => {
                let thread = args["threadId"].as_u64().unwrap_or(0);
                let moved = match self.engine() {
                    Ok(engine) => {
                        let _ = engine.select_task(thread);
                        match command.as_str() {
                            "next" => engine.move_line(true),
                            "stepIn" => engine.step(),
                            "stepBack" => engine.move_line(false),
                            "continue" => engine.run_to_breakpoint(true),
                            _ => engine.run_to_breakpoint(false),
                        }
                    }
                    Err(m) => return self.fail(request, &m).map(|_| false),
                };
                if let Err(error) = moved {
                    return self.fail(request, &error.to_string()).map(|_| false);
                }
                let body = if command == "continue" || command == "reverseContinue" {
                    json!({"allThreadsContinued":true})
                } else {
                    json!({})
                };
                self.respond(request, true, body, None)?;
                let forward = matches!(command.as_str(), "next" | "stepIn" | "continue");
                let reason = if command.contains("ontinue") {
                    "breakpoint"
                } else {
                    "step"
                };
                self.after_move(reason, thread, forward)?;
            }
            "stepOut" => {
                self.fail(
                    request,
                    "stepOut is not supported: a recording has no reliable frame-exit information",
                )?;
            }
            "pause" => {
                self.fail(
                    request,
                    "pause is not supported: the recording is already stopped",
                )?;
            }
            "source" => {
                let path = args["source"]["path"]
                    .as_str()
                    .or(args["source"]["name"].as_str())
                    .unwrap_or("");
                let root = self.root.clone();
                let candidate = root.join(path.trim_start_matches('/'));
                let allowed =
                    candidate.starts_with(&root) && !path.contains("..") && candidate.is_file();
                match allowed.then(|| fs::read_to_string(&candidate)).transpose() {
                    Ok(Some(text)) => self.respond(request, true, json!({"content":text}), None)?,
                    _ => self.fail(request, "source text is not available: the recording is source-free or the file is outside the root")?,
                }
            }
            "disconnect" => {
                self.respond(request, true, json!({}), None)?;
                return Ok(true);
            }
            other => {
                self.fail(request, &format!("unsupported request: {other}"))?;
            }
        }
        Ok(false)
    }
}

/// Serves one DAP client on stdin/stdout until `disconnect` or end of input.
pub fn serve(root: &Path, options: RunOptions) -> Result<()> {
    serve_on(&mut io::stdin().lock(), io::stdout().lock(), root, options)
}

pub fn serve_on(
    input: &mut impl BufRead,
    out: impl Write,
    root: &Path,
    options: RunOptions,
) -> Result<()> {
    let mut session = Session {
        out,
        seq: 0,
        root: root.to_path_buf(),
        options,
        engine: None,
        browser: Browser::new(),
    };
    loop {
        match read_message(input) {
            Ok(None) => return Ok(()),
            Ok(Some(message)) => {
                if message["type"] != "request" || message["command"].as_str().is_none() {
                    session.event(
                        "output",
                        json!({"category":"stderr","output":"ignored a message that is not a request\n"}),
                    )?;
                    continue;
                }
                if session.handle(&message)? {
                    return Ok(());
                }
            }
            Err(problem) => {
                // The stream is no longer trustworthy after a framing or size error.
                session.event(
                    "output",
                    json!({"category":"stderr","output":format!("DAP transport error: {problem}\n")}),
                )?;
                return Err(Error::InvalidOperation(format!(
                    "DAP transport error: {problem}"
                )));
            }
        }
    }
}
