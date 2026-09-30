use super::*;
#[derive(Clone)]
struct Occurrence {
    path: PathBuf,
    at: Tok,
    key: String,
    declaration: bool,
}
struct Index<'a> {
    p: &'a Program,
    sources: BTreeMap<PathBuf, String>,
    tokens: BTreeMap<PathBuf, Vec<Tok>>,
    uses: Vec<Occurrence>,
    definitions: BTreeMap<String, Occurrence>,
    scopes: Vec<BTreeMap<String, (String, String)>>,
    origin: PathBuf,
    bounds: BTreeMap<String, String>,
    pattern_cursors: BTreeMap<(PathBuf, usize, usize), usize>,
}
impl<'a> Index<'a> {
    fn add(&mut self, key: String, at: Tok, declaration: bool) {
        let o = Occurrence {
            path: self.origin.clone(),
            at,
            key: key.clone(),
            declaration,
        };
        if declaration {
            self.definitions.entry(key).or_insert_with(|| o.clone());
        }
        if !self.uses.iter().any(|old| {
            old.path == o.path
                && old.at.line == o.at.line
                && old.at.col == o.at.col
                && old.key == o.key
        }) {
            self.uses.push(o);
        }
    }
    fn token(&self, name: &str, at: &Tok) -> Option<Tok> {
        self.tokens
            .get(&self.origin)?
            .iter()
            .find(|t| (t.line, t.col) >= (at.line, at.col) && t.text == name)
            .cloned()
    }
    fn binding(&self, name: &str) -> Option<&(String, String)> {
        self.scopes.iter().rev().find_map(|s| s.get(name))
    }
    fn function_key(&self, symbol: &str) -> String {
        for ((trait_name, _), methods) in &self.p.impls {
            for (method, target) in methods {
                if target == symbol {
                    return format!("method:{trait_name}:{method}");
                }
            }
        }
        format!("global:{symbol}")
    }
    fn symbol(&self, name: &str) -> Option<String> {
        if let Some((key, _)) = self.binding(name) {
            return Some(key.clone());
        }
        let n = resolve_alias(self.p, name);
        let base = n.split('<').next().unwrap_or(&n);
        if self.p.functions.contains_key(base)
            || self.p.structs.contains_key(base)
            || self.p.enums.contains_key(base)
            || self.p.aliases.contains_key(base)
        {
            Some(format!("global:{base}"))
        } else {
            None
        }
    }
    fn checker(&self) -> Checker<'_> {
        Checker {
            program: self.p,
            scopes: self
                .scopes
                .iter()
                .map(|s| {
                    s.iter()
                        .map(|(n, (_, t))| (n.clone(), (t.clone(), false)))
                        .collect()
                })
                .collect(),
            return_ty: None,
            loop_depth: 0,
            bounds: self.bounds.clone(),
            origin: self.origin.clone(),
        }
    }
    fn bind(&mut self, name: &str, ty: String, at: &Tok) {
        if name.starts_with('$') && !name.starts_with("$import$") {
            return;
        }
        let raw = name.rsplit('$').next().unwrap_or(name);
        let Some(at) = self.token(raw, at) else {
            return;
        };
        let key = format!("local:{}:{}:{}", self.origin.display(), at.line, at.col);
        self.add(key.clone(), at, true);
        self.scopes
            .last_mut()
            .unwrap()
            .insert(name.into(), (key, ty));
    }
    fn expr(&mut self, e: &Expr) {
        match &e.kind {
            ExprKind::Name(n) => {
                if let Some(key) = self.symbol(n) {
                    self.add(key, e.at.clone(), false);
                }
            }
            ExprKind::Unary(_, e) | ExprKind::Try(e) => self.expr(e),
            ExprKind::Binary(_, a, b) => {
                self.expr(a);
                self.expr(b);
            }
            ExprKind::Call(t, args) => {
                self.expr(t);
                for a in args {
                    self.expr(a);
                }
            }
            ExprKind::Member(base, member) => {
                if let ExprKind::Name(n) = &base.kind {
                    if let Some(symbol) = self.p.import_aliases.get(&format!("{n}.{member}")) {
                        if let Some(at) = self.token(member, &e.at) {
                            self.add(format!("global:{symbol}"), at, false);
                        }
                        return;
                    }
                }
                let ty = self.checker().expr(base).ok();
                self.expr(base);
                if let Some(ty) = ty {
                    if let Some(bound) = self.bounds.get(v06::borrowed_type(&ty)) {
                        if self
                            .p
                            .traits
                            .get(bound)
                            .is_some_and(|t| t.methods.contains_key(member))
                        {
                            if let Some(at) = self.token(member, &e.at) {
                                self.add(format!("method:{bound}:{member}"), at, false);
                            }
                        }
                    }
                    let methods = matching_methods(self.p, &ty, member);
                    if methods.len() == 1 {
                        if let Some(at) = self.token(member, &e.at) {
                            self.add(self.function_key(&methods[0]), at, false);
                        }
                    }
                }
            }
            ExprKind::NamedConstructor(n, fields) => {
                if let Some(key) = self.symbol(n.split("::").next().unwrap_or(n)) {
                    self.add(key, e.at.clone(), false);
                }
                for (_, e) in fields {
                    self.expr(e);
                }
            }
            ExprKind::Closure(params, _, body) => {
                self.scopes.push(BTreeMap::new());
                let mut at = e.at.clone();
                for (n, t) in params {
                    self.bind(n, t.clone(), &at);
                    if let Some(next) = self.token(n, &at) {
                        at = next;
                        at.col += n.chars().count();
                    }
                }
                self.body(body);
                self.scopes.pop();
            }
            ExprKind::Match(value, arms) => {
                let ty = self.checker().expr(value).unwrap_or_default();
                self.expr(value);
                for (p, g, e) in arms {
                    self.scopes.push(BTreeMap::new());
                    self.pattern(p, &ty, &value.at);
                    if let Some(g) = g {
                        self.expr(g);
                    }
                    self.expr(e);
                    self.advance_pattern(
                        &value.at,
                        &[Stmt {
                            kind: StmtKind::Expr(e.clone()),
                            at: e.at.clone(),
                        }],
                    );
                    self.scopes.pop();
                }
            }
            _ => {}
        }
    }
    fn pattern(&mut self, p: &Pattern, ty: &str, at: &Tok) {
        let mut vars = BTreeMap::new();
        let _ = self.checker().pattern(p, ty, at, &mut vars);
        let tokens = &self.tokens[&self.origin];
        let key = (self.origin.clone(), at.line, at.col);
        let start = self.pattern_cursors.get(&key).copied().unwrap_or_else(|| {
            tokens
                .iter()
                .position(|t| (t.line, t.col) >= (at.line, at.col) && t.text == "{")
                .map(|i| i + 1)
                .unwrap_or(tokens.len())
        });
        let Some(arrow) = (start..tokens.len()).find(|i| tokens[*i].text == "=>") else {
            return;
        };
        self.pattern_cursors.insert(key, arrow + 1);
        let mut depth = 0;
        let mut begin = start;
        for i in (start..arrow).rev() {
            match tokens[i].text.as_str() {
                ")" => depth += 1,
                "(" => depth -= 1,
                "{" | "}" | "," if depth == 0 => {
                    begin = i + 1;
                    break;
                }
                _ => {}
            }
        }
        let bindings = vars
            .into_iter()
            .filter_map(|(n, (ty, _))| {
                tokens[begin..arrow]
                    .iter()
                    .find(|t| t.text == n)
                    .cloned()
                    .map(|at| (n, ty, at))
            })
            .collect::<Vec<_>>();
        for (n, ty, at) in bindings {
            self.bind(&n, ty, &at);
        }
    }
    fn advance_pattern(&mut self, at: &Tok, body: &[Stmt]) {
        let mut last = (at.line, at.col);
        v05::expressions(body, &mut |e| {
            last = last.max((e.at.line, e.at.col));
        });
        let cursor = self.tokens[&self.origin]
            .iter()
            .position(|t| (t.line, t.col) > last)
            .unwrap_or(self.tokens[&self.origin].len());
        let key = (self.origin.clone(), at.line, at.col);
        if let Some(old) = self.pattern_cursors.get_mut(&key) {
            *old = (*old).max(cursor);
        }
    }
    fn scope(&mut self, b: &[Stmt]) {
        self.scopes.push(BTreeMap::new());
        self.body(b);
        self.scopes.pop();
    }
    fn body(&mut self, b: &[Stmt]) {
        for s in b {
            match &s.kind {
                StmtKind::Let(n, _, _, e) | StmtKind::Using(n, e) => {
                    let ty = match &s.kind {
                        StmtKind::Let(_, _, Some(t), _) => t.clone(),
                        _ => self.checker().expr(e).unwrap_or_else(|_| "Unknown".into()),
                    };
                    self.expr(e);
                    self.bind(n, ty, &s.at);
                }
                StmtKind::Expr(e) | StmtKind::Defer(e) | StmtKind::Return(Some(e)) => self.expr(e),
                StmtKind::Assign(a, _, b) => {
                    self.expr(a);
                    self.expr(b);
                }
                StmtKind::Block(b) | StmtKind::Branch(_, b) => self.scope(b),
                StmtKind::If(e, a, b) => {
                    self.expr(e);
                    self.scope(a);
                    self.scope(b);
                }
                StmtKind::While(e, b) => {
                    self.expr(e);
                    self.scope(b);
                }
                StmtKind::For(n, a, b, body) => {
                    self.expr(a);
                    self.expr(b);
                    self.scopes.push(BTreeMap::new());
                    self.bind(n, "Int".into(), &s.at);
                    self.body(body);
                    self.scopes.pop();
                }
                StmtKind::Match(e, arms) => {
                    let ty = self.checker().expr(e).unwrap_or_default();
                    self.expr(e);
                    for (p, g, b) in arms {
                        self.scopes.push(BTreeMap::new());
                        self.pattern(p, &ty, &e.at);
                        if let Some(g) = g {
                            self.expr(g);
                        }
                        self.body(std::slice::from_ref(b));
                        self.advance_pattern(&e.at, std::slice::from_ref(b));
                        self.scopes.pop();
                    }
                }
                _ => {}
            }
        }
    }
    fn new(p: &'a Program, documents: &BTreeMap<PathBuf, String>) -> Result<Self> {
        let mut sources = BTreeMap::new();
        let mut tokens = BTreeMap::new();
        for path in &p.included_modules {
            let source = documents
                .get(path)
                .cloned()
                .map(Ok)
                .unwrap_or_else(|| fs::read_to_string(path))?;
            tokens.insert(path.clone(), lex(&source)?);
            sources.insert(path.clone(), source);
        }
        let mut index = Self {
            p,
            sources,
            tokens,
            uses: Vec::new(),
            definitions: BTreeMap::new(),
            scopes: vec![BTreeMap::new()],
            origin: p.root_origin.clone(),
            bounds: BTreeMap::new(),
            pattern_cursors: BTreeMap::new(),
        };
        for (n, f) in &p.functions {
            if n.starts_with("$closure") {
                continue;
            }
            index.origin = f.origin.clone();
            let name = index
                .tokens
                .get(&f.origin)
                .and_then(|ts| {
                    ts.iter()
                        .position(|t| t.line == f.at.line && t.col == f.at.col)
                        .and_then(|i| ts.get(i + 1))
                })
                .cloned();
            if let Some(at) = name {
                index.add(index.function_key(n), at, true);
            }
        }
        // Preserve type locations from tokens before alias expansion erases their spellings.
        for (path, tokens) in index.tokens.clone() {
            index.origin = path.clone();
            for (i, token) in tokens.iter().enumerate() {
                let declaration = i > 0
                    && matches!(
                        tokens[i - 1].text.as_str(),
                        "struct" | "enum" | "trait" | "type"
                    );
                let type_name = resolve_alias(p, &token.text);
                let is_type = p.structs.contains_key(&type_name)
                    || p.enums.contains_key(&type_name)
                    || p.traits.contains_key(&type_name)
                    || p.aliases.contains_key(&type_name);
                let in_type = i > 0
                    && matches!(
                        tokens[i - 1].text.as_str(),
                        ":" | "->" | "<" | "for" | "impl" | "&"
                    );
                if is_type && (declaration || in_type) {
                    index.add(format!("global:{type_name}"), token.clone(), declaration);
                }
            }
            for (name, tr) in &p.traits {
                if tr.origin != path {
                    continue;
                }
                let Some(begin) = tokens.windows(2).position(|ts| {
                    ts[0].text == "trait" && ts[1].text == name.rsplit('$').next().unwrap_or(name)
                }) else {
                    continue;
                };
                let mut braces = 0;
                let mut started = false;
                for i in begin + 2..tokens.len() {
                    if tokens[i].text == "{" {
                        braces += 1;
                        started = true;
                    }
                    if tokens[i].text == "}" {
                        braces -= 1;
                        if started && braces == 0 {
                            break;
                        }
                    }
                    if braces == 1 && tokens[i].text == "fn" {
                        if let Some(method) = tokens.get(i + 1) {
                            if tr.methods.contains_key(&method.text) {
                                let key = format!("method:{name}:{}", method.text);
                                index.definitions.remove(&key);
                                index.add(key, method.clone(), true);
                            }
                        }
                    }
                }
            }
        }
        for (stmt, origin) in p.stmts.iter().zip(&p.stmt_origins) {
            index.origin = origin.clone();
            index.body(std::slice::from_ref(stmt));
        }
        let globals = index.scopes.clone();
        for f in p.functions.values() {
            if f.origin.as_os_str().is_empty() {
                continue;
            }
            index.origin = f.origin.clone();
            index.bounds = f
                .type_params
                .iter()
                .filter_map(|(n, b)| b.as_ref().map(|b| (n.clone(), b.clone())))
                .collect();
            index.pattern_cursors.clear();
            index.scopes = globals.clone();
            index.scopes.push(BTreeMap::new());
            let mut at = f.at.clone();
            for (n, t) in &f.params {
                index.bind(n, t.clone(), &at);
                if let Some(next) = index.token(n, &at) {
                    at = next;
                    at.col += n.chars().count();
                }
            }
            index.body(&f.body);
        }
        Ok(index)
    }
    fn range(&self, o: &Occurrence) -> Json {
        let source = &self.sources[&o.path];
        let line = o.at.line.saturating_sub(1);
        let col = utf16_column(source, line, o.at.col.saturating_sub(1));
        json!({"start":{"line":line,"character":col},"end":{"line":line,"character":col+o.at.text.encode_utf16().count()}})
    }
    fn location(&self, o: &Occurrence) -> Json {
        json!({"uri":file_uri(&o.path),"range":self.range(o)})
    }
    fn at(&self, path: &Path, pos: &Json) -> Result<Option<&Occurrence>> {
        let text = &self.sources[path];
        let offset = utf16_offset(text, pos)?;
        let line = text[..offset].bytes().filter(|b| *b == b'\n').count() + 1;
        let col = text[..offset]
            .rsplit('\n')
            .next()
            .unwrap_or("")
            .chars()
            .count()
            + 1;
        Ok(self.uses.iter().find(|o| {
            o.path == path
                && o.at.line == line
                && o.at.col <= col
                && o.at.col + o.at.text.chars().count() > col
        }))
    }
}
pub(super) fn request(
    root: &Path,
    method: &str,
    p: &Program,
    path: &Path,
    params: &Json,
    documents: &BTreeMap<PathBuf, String>,
) -> Result<Json> {
    let index = Index::new(p, documents)?;
    if method.ends_with("signatureHelp") {
        let text = &index.sources[path];
        let offset = utf16_offset(text, &params["position"])?;
        let tokens = lex(&text[..offset])?;
        let mut stack: Vec<(usize, usize)> = Vec::new();
        for (i, t) in tokens.iter().enumerate() {
            match t.text.as_str() {
                "(" => stack.push((i, 0)),
                ")" => {
                    stack.pop();
                }
                "," => {
                    if let Some((_, count)) = stack.last_mut() {
                        *count += 1;
                    }
                }
                _ => {}
            }
        }
        let Some((open, active)) = stack.last().copied() else {
            return Ok(Json::Null);
        };
        let Some(token) = open.checked_sub(1).and_then(|i| tokens.get(i)) else {
            return Ok(Json::Null);
        };
        let position =
            json!({"line":token.line-1,"character":utf16_column(text,token.line-1,token.col-1)});
        let symbol = index
            .at(path, &position)?
            .map(|o| o.key.trim_start_matches("global:").to_string())
            .unwrap_or_else(|| resolve_alias(p, &token.text));
        let is_method = symbol.starts_with("method:");
        let symbol = if is_method {
            index
                .uses
                .iter()
                .find(|o| o.key == symbol && o.declaration)
                .and_then(|o| {
                    p.functions.iter().find(|(_, f)| {
                        f.origin == o.path && f.at.line == o.at.line && f.at.col < o.at.col
                    })
                })
                .map(|(n, _)| n.clone())
                .unwrap_or(symbol)
        } else {
            symbol
        };
        let Some(f) = p.functions.get(&symbol) else {
            return Ok(Json::Null);
        };
        let parameters = f
            .params
            .iter()
            .map(|(n, t)| json!({"label":format!("{n}: {t}")}))
            .collect::<Vec<_>>();
        let label = format!(
            "{}({})->{} effects {:?}",
            token.text,
            f.params
                .iter()
                .skip(usize::from(is_method))
                .map(|(n, t)| format!("{n}: {t}"))
                .collect::<Vec<_>>()
                .join(", "),
            f.ret,
            f.effects
        );
        return Ok(
            json!({"signatures":[{"label":label,"parameters":parameters}],"activeSignature":0,"activeParameter":active.min(f.params.len().saturating_sub(usize::from(is_method)+1))}),
        );
    }
    let Some(target) = index.at(path, &params["position"])? else {
        return Ok(Json::Null);
    };
    let occurrences = index
        .uses
        .iter()
        .filter(|o| o.key == target.key)
        .collect::<Vec<_>>();
    if method.ends_with("references") {
        return Ok(Json::Array(
            occurrences
                .iter()
                .filter(|o| params["context"]["includeDeclaration"] == true || !o.declaration)
                .map(|o| index.location(o))
                .collect(),
        ));
    }
    if method.ends_with("definition") {
        return Ok(index
            .definitions
            .get(&target.key)
            .map(|o| index.location(o))
            .unwrap_or(Json::Null));
    }
    if method.ends_with("prepareRename") {
        return Ok(json!({"range":index.range(target),"placeholder":target.at.text}));
    }
    if method.ends_with("rename") {
        if occurrences.iter().any(|o| o.at.text != target.at.text) {
            return Err(Error::InvalidOperation("rename across selective import aliases requires renaming the original export separately".into()));
        }
        let name = params["newName"]
            .as_str()
            .ok_or_else(|| Error::InvalidOperation("newName required".into()))?;
        let tokens = lex(name)?;
        if tokens.len() != 2
            || tokens[0].text != name
            || !name
                .chars()
                .next()
                .is_some_and(|c| c.is_alphabetic() || c == '_')
            || [
                "let", "var", "fn", "type", "struct", "enum", "trait", "impl", "return", "if",
                "else", "while", "for", "match", "true", "false", "move", "capture", "effects",
                "await", "spawn", "commit", "revert", "resume", "publish", "defer", "using",
                "break", "continue", "async", "pub", "test", "import", "from", "as", "freeze",
                "thaw", "in", "captures",
            ]
            .contains(&name)
        {
            return Err(Error::InvalidOperation(
                "rename requires an identifier".into(),
            ));
        }
        if index
            .uses
            .iter()
            .any(|o| o.declaration && o.at.text == name && o.key != target.key)
        {
            return Err(Error::InvalidOperation(
                "rename would conflict with an existing declaration".into(),
            ));
        }
        let mut changes: BTreeMap<String, Vec<Json>> = BTreeMap::new();
        for o in &occurrences {
            changes
                .entry(file_uri(&o.path))
                .or_default()
                .push(json!({"range":index.range(o),"newText":name}));
        }
        let mut edited = documents.clone();
        for (uri, edits) in &changes {
            let path = document_path(uri)?;
            let text = index.sources[&path].clone();
            let mut next = text.clone();
            let mut ranges = edits
                .iter()
                .map(|edit| {
                    Ok((
                        utf16_offset(&text, &edit["range"]["start"])?,
                        utf16_offset(&text, &edit["range"]["end"])?,
                    ))
                })
                .collect::<Result<Vec<_>>>()?;
            ranges.sort_by(|a, b| b.0.cmp(&a.0));
            for (start, end) in ranges {
                next.replace_range(start..end, name);
            }
            edited.insert(path, next);
        }
        workspace_program(root, path, &edited).map_err(|e| {
            Error::InvalidOperation(format!("rename cannot preserve the checked workspace: {e}"))
        })?;
        return Ok(json!({"changes":changes}));
    }
    Ok(Json::Null)
}

pub(super) fn diagnostics(root: &Path, path: &Path, text: &str, error: &Error) -> Vec<Json> {
    let d = match error {
        Error::Diagnostic(d) => Some(d.as_ref()),
        _ => None,
    };
    let location = d
        .map(|d| (d.line.saturating_sub(1), d.column.saturating_sub(1)))
        .unwrap_or((0, 0));
    let target = d
        .filter(|d| !d.source.is_empty())
        .map(|d| root.join(&d.source))
        .unwrap_or_else(|| path.into());
    let source = if target == path {
        text.to_string()
    } else {
        fs::read_to_string(&target).unwrap_or_default()
    };
    let col = utf16_column(&source, location.0, location.1);
    let mut result = json!({"range":{"start":{"line":location.0,"character":col},"end":{"line":location.0,"character":col+1}},"severity":1,"source":"rewind","message":error.to_string(),"code":d.map(|d|d.code.as_str()).unwrap_or("Error")});
    if let Some(d) = d {
        fn causes<'a>(
            d: &'a rewind::DiagnosticRecord,
            out: &mut Vec<&'a rewind::DiagnosticRecord>,
            depth: usize,
        ) {
            if depth >= 128 {
                return;
            }
            for cause in &d.causes {
                out.push(cause);
                causes(cause, out, depth + 1);
            }
        }
        let mut nested = Vec::new();
        causes(d, &mut nested, 0);
        let related=nested.into_iter().map(|cause|{
            let path=root.join(&cause.source);let text=fs::read_to_string(&path).unwrap_or_default();let line=cause.line.saturating_sub(1);let col=utf16_column(&text,line,cause.column.saturating_sub(1));
            json!({"location":{"uri":file_uri(&path),"range":{"start":{"line":line,"character":col},"end":{"line":line,"character":col+1}}},"message":cause.message})
        }).collect::<Vec<_>>();
        result["relatedInformation"] = json!(related);
    }
    if target != path {
        let range = result["range"].clone();
        result["relatedInformation"].as_array_mut().map(|v|v.push(json!({"location":{"uri":file_uri(&target),"range":range},"message":"diagnostic originates in this module"})));
        result["range"] = json!({"start":{"line":0,"character":0},"end":{"line":0,"character":0}});
    }
    vec![result]
}
pub(super) fn quick_fixes(
    root: &Path,
    params: &Json,
    documents: &BTreeMap<PathBuf, String>,
) -> Result<Json> {
    let uri = params["textDocument"]["uri"].as_str().unwrap_or("");
    let path = document_path(uri)?;
    if !path.starts_with(fs::canonicalize(root)?) {
        return Err(Error::InvalidPath(path.display().to_string()));
    }
    let text = documents
        .get(&path)
        .cloned()
        .map(Ok)
        .unwrap_or_else(|| fs::read_to_string(&path))?;
    let tokens = lex(&text)?;
    let mut fixes = Vec::new();
    for d in params["context"]["diagnostics"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let message = d["message"].as_str().unwrap_or("");
        let start = &d["range"]["start"];
        let _ = utf16_offset(&text, start)?;
        let mut edit = None;
        let mut title = String::new();
        if message.contains("requires move or freeze") || message.contains("requires move, &") {
            edit = Some(json!({"range":{"start":start,"end":start},"newText":"move "}));
            title = "Move this owner".into();
        } else if message.contains("undeclared effect ")
            || message.contains("public function requires effects")
        {
            let effect = message
                .split("undeclared effect ")
                .nth(1)
                .and_then(|s| s.split_whitespace().next())
                .unwrap_or("");
            let line = start["line"].as_u64().unwrap_or(0) as usize + 1;
            if let Some(begin) = tokens
                .iter()
                .rposition(|t| t.text == "fn" && t.line <= line)
            {
                let mut i = begin + 1;
                let mut effects = None;
                let mut body = None;
                while i < tokens.len() {
                    if tokens[i].text == "effects" {
                        effects = Some(i);
                        if tokens.get(i + 1).is_some_and(|t| t.text == "{") {
                            i += 2;
                            while i < tokens.len() && tokens[i].text != "}" {
                                i += 1;
                            }
                        } else {
                            i += 1;
                        }
                    } else if tokens[i].text == "{" {
                        body = Some(i);
                        break;
                    }
                    i += 1;
                }
                let position = |t: &Tok| json!({"line":t.line-1,"character":utf16_column(&text,t.line-1,t.col-1)});
                if let Some(at) = effects {
                    if !effect.is_empty() && tokens.get(at + 1).is_some_and(|t| t.text == "{") {
                        let close = (at + 2..tokens.len())
                            .find(|i| tokens[*i].text == "}")
                            .unwrap();
                        let pos = position(&tokens[close]);
                        let prefix = if close == at + 2 { "" } else { "," };
                        edit = Some(
                            json!({"range":{"start":pos,"end":pos},"newText":format!("{prefix}{effect}")}),
                        );
                    }
                } else if let Some(at) = body {
                    let pos = position(&tokens[at]);
                    edit = Some(
                        json!({"range":{"start":pos,"end":pos},"newText":format!("effects {{{effect}}} ")}),
                    );
                }
                title = "Declare the required effects".into();
            }
        }
        if let Some(edit) = edit {
            fixes.push(json!({"title":title,"kind":"quickfix","diagnostics":[d],"edit":{"changes":{uri:[edit]}}}));
        }
    }
    Ok(json!(fixes))
}
