//! Bounded browsing of recorded values (DBG-02).
//!
//! A debug recording stores each variable as the text the runtime rendered (secrets
//! already masked). This module turns that text into lazily expanded pages: it never
//! stringifies a graph again, never runs program code, and invalidates every reference
//! when the debugger moves to another event. Children are derived from the rendered
//! text, so a text value that itself contains list or map punctuation can be split
//! imprecisely; `ty` and `value` always say what was recorded.
use serde_json::Value as Json;
use std::collections::HashMap;

const MAX_VALUE_CHARS: usize = 120;
const MAX_NODE_BYTES: usize = 1024 * 1024;
const MAX_CHILDREN: usize = 65_536;
const MAX_PATH: usize = 64;
const MAX_REFERENCES: usize = 16_384;

#[derive(Clone, Debug, PartialEq)]
pub struct Var {
    pub name: String,
    pub value: String,
    pub ty: String,
    /// 0 when the value has nothing to expand.
    pub reference: u64,
    pub total: usize,
}

enum Node {
    Scope(Vec<(String, String)>),
    Value { text: String, path: Vec<u64> },
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Shape {
    Scalar,
    List,
    Map,
    Record,
    Variant,
    Object(u64),
}

pub struct Browser {
    generation: u64,
    next: u64,
    nodes: HashMap<u64, Node>,
}

impl Default for Browser {
    fn default() -> Self {
        Self::new()
    }
}

fn heap_id(text: &str) -> Option<u64> {
    let inner = text
        .strip_prefix("<object:")
        .and_then(|s| s.strip_suffix('>'))
        .or_else(|| {
            text.strip_prefix("HeapRef(")
                .and_then(|s| s.strip_suffix(')'))
        })?;
    inner.trim().parse().ok()
}

/// Splits `text` at top-level occurrences of `separator` (outside brackets and quotes).
fn split_top(text: &str, separator: char, limit: usize) -> Vec<&str> {
    let mut parts = Vec::new();
    let (mut depth, mut start) = (0i32, 0usize);
    let (mut quoted, mut escaped) = (false, false);
    for (index, c) in text.char_indices() {
        if quoted {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                quoted = false;
            }
            continue;
        }
        match c {
            '"' => quoted = true,
            '[' | '{' | '(' => depth += 1,
            ']' | '}' | ')' => depth -= 1,
            _ if c == separator && depth == 0 => {
                parts.push(text[start..index].trim());
                start = index + c.len_utf8();
                if parts.len() >= limit {
                    return parts;
                }
            }
            _ => {}
        }
    }
    let rest = text[start..].trim();
    if !rest.is_empty() || !parts.is_empty() {
        parts.push(rest);
    }
    parts
}

fn strip_wrapper<'a>(text: &'a str, open: &str, close: &str) -> Option<&'a str> {
    text.strip_prefix(open)?.strip_suffix(close)
}

fn shape_of(text: &str) -> (Shape, String) {
    if let Some(id) = heap_id(text) {
        return (Shape::Object(id), "object".into());
    }
    if let Some(inner) = strip_wrapper(text, "List([", "])") {
        let _ = inner;
        return (Shape::List, "List".into());
    }
    if text.starts_with('[') && text.ends_with(']') {
        return (Shape::List, "List".into());
    }
    if text.starts_with('{') && text.ends_with('}') {
        return (Shape::Map, "Map".into());
    }
    if let Some(rest) = text.strip_prefix("Map(") {
        if rest.ends_with(')') {
            return (Shape::Map, "Map".into());
        }
    }
    for wrapper in ["Some", "Ok", "Err"] {
        if strip_wrapper(text, &format!("{wrapper}("), ")").is_some() {
            return (Shape::Variant, wrapper.into());
        }
    }
    if let Some(rest) = text.strip_prefix("Struct(\"") {
        if let Some(end) = rest.find('"') {
            return (Shape::Record, rest[..end].into());
        }
    }
    if let Some(open) = text.find('{') {
        let name = &text[..open];
        if !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_alphanumeric() || matches!(c, '_' | '<' | '>' | ',' | ' '))
            && text.ends_with('}')
        {
            return (Shape::Record, name.trim().into());
        }
    }
    if let Some((name, rest)) = text.split_once("::") {
        if !name.is_empty()
            && name.chars().all(|c| c.is_alphanumeric() || c == '_')
            && rest.chars().next().is_some_and(|c| c.is_alphabetic())
        {
            return (Shape::Variant, name.into());
        }
    }
    (Shape::Scalar, scalar_type(text).into())
}

fn scalar_type(text: &str) -> &'static str {
    if text.starts_with("Int(") || text.parse::<i64>().is_ok() {
        "Int"
    } else if text.starts_with("Float(") || (text.parse::<f64>().is_ok() && text.contains('.')) {
        "Float"
    } else if text.starts_with("Text(") {
        "String"
    } else if text.starts_with("Bool(") || text == "true" || text == "false" {
        "Bool"
    } else if text.starts_with('<') {
        "opaque"
    } else if text == "None" || text == "null" {
        "Option"
    } else {
        "String"
    }
}

fn decode_scalar(text: &str) -> String {
    if let Some(n) = strip_wrapper(text, "Int(", ")") {
        return n.into();
    }
    if let Some(s) = strip_wrapper(text, "Text(", ")") {
        return s.into();
    }
    if let Some(b) = strip_wrapper(text, "Bool(", ")") {
        return b.into();
    }
    if let Some(bits) = strip_wrapper(text, "Float(", ")") {
        if let Ok(bits) = bits.trim().parse::<u64>() {
            return f64::from_bits(bits).to_string();
        }
    }
    text.into()
}

fn shorten(text: &str) -> String {
    let count = text.chars().count();
    if count <= MAX_VALUE_CHARS {
        text.into()
    } else {
        format!(
            "{}… ({count} characters)",
            text.chars().take(MAX_VALUE_CHARS).collect::<String>()
        )
    }
}

/// The child (name, text) pairs of a container, bounded by MAX_CHILDREN.
fn children_of(shape: Shape, text: &str, heap: &Json) -> Vec<(String, String)> {
    let inner_of = |open: &str, close: &str| strip_wrapper(text, open, close).map(str::to_string);
    match shape {
        Shape::List => {
            let inner = inner_of("List([", "])").unwrap_or_else(|| {
                text.trim_start_matches('[')
                    .trim_end_matches(']')
                    .to_string()
            });
            split_top(&inner, ',', MAX_CHILDREN)
                .into_iter()
                .filter(|s| !s.is_empty())
                .enumerate()
                .map(|(i, s)| (format!("[{i}]"), s.to_string()))
                .collect()
        }
        Shape::Map => {
            let inner = inner_of("Map({", "})")
                .or_else(|| inner_of("{", "}"))
                .unwrap_or_default();
            split_top(&inner, ',', MAX_CHILDREN)
                .into_iter()
                .filter(|s| !s.is_empty())
                .map(|pair| match split_top(pair, ':', 2).as_slice() {
                    [k, v] => (decode_scalar(k), v.to_string()),
                    _ => (pair.to_string(), String::new()),
                })
                .collect()
        }
        Shape::Record => {
            let open = text.find('{').unwrap_or(0);
            let inner = text[open + 1..].trim_end_matches(')').trim_end_matches('}');
            split_top(inner, ',', MAX_CHILDREN)
                .into_iter()
                .filter(|s| !s.is_empty())
                .map(|field| match split_top(field, ':', 2).as_slice() {
                    [k, v] => (k.trim_matches('"').to_string(), v.to_string()),
                    _ => (field.to_string(), String::new()),
                })
                .collect()
        }
        Shape::Variant => {
            for wrapper in ["Some", "Ok", "Err"] {
                if let Some(inner) = strip_wrapper(text, &format!("{wrapper}("), ")") {
                    return vec![("value".into(), inner.to_string())];
                }
            }
            // Name::Variant[("field", value), ...]
            let fields = text
                .find('[')
                .map(|open| text[open + 1..].trim_end_matches(']'))
                .unwrap_or("");
            split_top(fields, ',', MAX_CHILDREN)
                .into_iter()
                .filter(|s| !s.is_empty())
                .filter_map(|tuple| {
                    let inner = tuple.strip_prefix('(')?.strip_suffix(')')?;
                    match split_top(inner, ',', 2).as_slice() {
                        [k, v] => Some((k.trim_matches('"').to_string(), v.to_string())),
                        _ => None,
                    }
                })
                .collect()
        }
        Shape::Object(id) => heap[id.to_string()]
            .as_str()
            .map(|t| vec![("object".to_string(), t.to_string())])
            .unwrap_or_default(),
        Shape::Scalar => Vec::new(),
    }
}

impl Browser {
    pub fn new() -> Self {
        Self {
            generation: 0,
            next: 1,
            nodes: HashMap::new(),
        }
    }
    /// Invalidates every reference handed out so far (the debugger moved).
    pub fn reset(&mut self) {
        self.generation += 1;
        self.nodes.clear();
        self.next = 1;
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    fn alloc(&mut self, node: Node) -> u64 {
        if self.nodes.len() >= MAX_REFERENCES {
            return 0;
        }
        let id = self.next;
        self.next += 1;
        self.nodes.insert(id, node);
        id
    }
    /// A reference for one lexical scope (name → rendered value). Returns (reference, count).
    pub fn scope(&mut self, entries: Vec<(String, String)>) -> (u64, usize) {
        let count = entries.len();
        (self.alloc(Node::Scope(entries)), count)
    }
    fn describe(&mut self, name: String, text: &str, path: &[u64], heap: &Json) -> Var {
        if text.len() > MAX_NODE_BYTES {
            return Var {
                name,
                value: format!("<large value, {} bytes not expanded>", text.len()),
                ty: "opaque".into(),
                reference: 0,
                total: 0,
            };
        }
        let (shape, ty) = shape_of(text);
        if let Shape::Object(id) = shape {
            if path.contains(&id) {
                return Var {
                    name,
                    value: format!("<cycle to object {id}>"),
                    ty: "cycle".into(),
                    reference: 0,
                    total: 0,
                };
            }
            if path.len() < MAX_PATH {
                if let Some(target) = heap[id.to_string()].as_str() {
                    let mut next_path = path.to_vec();
                    next_path.push(id);
                    let mut var = self.describe(name, target, &next_path, heap);
                    var.value = format!("{} (object {id})", var.value);
                    return var;
                }
            }
            return Var {
                name,
                value: format!("<object {id} not recorded>"),
                ty: "object".into(),
                reference: 0,
                total: 0,
            };
        }
        if shape == Shape::Scalar {
            return Var {
                name,
                value: shorten(&decode_scalar(text)),
                ty,
                reference: 0,
                total: 0,
            };
        }
        let total = children_of(shape, text, heap).len();
        let reference = if total > 0 && path.len() < MAX_PATH {
            self.alloc(Node::Value {
                text: text.into(),
                path: path.to_vec(),
            })
        } else {
            0
        };
        let label = match shape {
            Shape::List => format!("{ty}, {total} items"),
            Shape::Map => format!("{ty}, {total} entries"),
            _ => ty,
        };
        Var {
            name,
            value: shorten(text),
            ty: label,
            reference,
            total,
        }
    }
    /// A page of the variables of a scope or the children of a value.
    pub fn variables(
        &mut self,
        reference: u64,
        start: usize,
        count: usize,
        heap: &Json,
    ) -> Result<Vec<Var>, String> {
        let count = count.clamp(1, 1000);
        let (entries, path): (Vec<(String, String)>, Vec<u64>) = match self.nodes.get(&reference) {
            None => {
                return Err(format!(
                    "stale or unknown variablesReference {reference}: references are valid only at the event where they were issued"
                ))
            }
            Some(Node::Scope(entries)) => (entries.clone(), Vec::new()),
            Some(Node::Value { text, path }) => {
                let (shape, _) = shape_of(text);
                (children_of(shape, text, heap), path.clone())
            }
        };
        Ok(entries
            .into_iter()
            .skip(start)
            .take(count)
            .map(|(name, text)| self.describe(name, &text, &path, heap))
            .collect())
    }
    /// Evaluates a read-only watch expression: `name(.field | [index])*`. Calls,
    /// operators, assignments, await and everything else are rejected.
    pub fn watch(
        &mut self,
        expression: &str,
        scopes: &[Vec<(String, String)>],
        heap: &Json,
    ) -> Result<Var, String> {
        let expression = expression.trim();
        if expression.is_empty() || expression.len() > 256 {
            return Err("watch expression is empty or too long".into());
        }
        let mut steps: Vec<(bool, String)> = Vec::new(); // (is_index, text)
        let mut chars = expression.chars().peekable();
        let mut head = String::new();
        while let Some(c) = chars.peek().copied() {
            if c.is_alphanumeric() || c == '_' || c == '$' {
                head.push(c);
                chars.next();
            } else {
                break;
            }
        }
        if head.is_empty() || head.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            return Err("a watch expression starts with a variable name".into());
        }
        while let Some(c) = chars.next() {
            match c {
                '.' => {
                    let mut field = String::new();
                    while let Some(n) = chars.peek().copied() {
                        if n.is_alphanumeric() || n == '_' {
                            field.push(n);
                            chars.next();
                        } else {
                            break;
                        }
                    }
                    if field.is_empty() {
                        return Err("expected a field name after '.'".into());
                    }
                    steps.push((false, field));
                }
                '[' => {
                    let mut digits = String::new();
                    for n in chars.by_ref() {
                        if n == ']' {
                            break;
                        }
                        digits.push(n);
                    }
                    if digits.is_empty() || !digits.chars().all(|d| d.is_ascii_digit()) {
                        return Err("an index must be a non-negative integer literal".into());
                    }
                    steps.push((true, digits));
                }
                _ => {
                    return Err(
                        "watch expressions may only read variables, fields and checked indexes: no calls, operators, assignment, await, publish or external operations"
                            .into(),
                    )
                }
            }
        }
        let found = scopes
            .iter()
            .rev()
            .flat_map(|scope| scope.iter().rev())
            .find(|(name, _)| *name == head)
            .map(|(_, text)| text.clone())
            .ok_or_else(|| format!("{head} is not in scope at this event"))?;
        let mut var = self.describe(head.clone(), &found, &[], heap);
        for (is_index, step) in steps {
            if var.reference == 0 {
                return Err(format!(
                    "{} has no {}",
                    var.name,
                    if is_index { "index" } else { "field" }
                ));
            }
            let want = if is_index {
                format!("[{step}]")
            } else {
                step.clone()
            };
            let page = self.variables(var.reference, 0, 1000, heap)?;
            var = page.into_iter().find(|v| v.name == want).ok_or_else(|| {
                format!(
                    "{} has no {}{}",
                    var.name,
                    if is_index { "item " } else { "field " },
                    step
                )
            })?;
        }
        var.name = expression.into();
        Ok(var)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn lists_maps_and_records_page_lazily() {
        let heap = json!({});
        let mut b = Browser::new();
        let (scope, _) = b.scope(vec![
            ("xs".into(), "[1, 2, 3]".into()),
            ("m".into(), "{Text(\"a\"): Int(1)}".into()),
            ("p".into(), "Point{\"x\": Int(3), \"y\": Int(4)}".into()),
        ]);
        let vars = b.variables(scope, 0, 10, &heap).unwrap();
        assert_eq!(vars[0].ty, "List, 3 items");
        assert_eq!(vars[1].ty, "Map, 1 entries");
        assert_eq!(vars[2].ty, "Point");
        let page = b.variables(vars[0].reference, 1, 1, &heap).unwrap();
        assert_eq!(page.len(), 1);
        assert_eq!(page[0].name, "[1]");
        assert_eq!(page[0].value, "2");
        let fields = b.variables(vars[2].reference, 0, 10, &heap).unwrap();
        assert_eq!(fields[0].name, "x");
        assert_eq!(fields[0].value, "3");
    }

    #[test]
    fn cycles_and_stale_references_are_reported() {
        let heap = json!({"1": "Node{\"next\": HeapRef(1)}"});
        let mut b = Browser::new();
        let (scope, _) = b.scope(vec![("n".into(), "<object:1>".into())]);
        let vars = b.variables(scope, 0, 10, &heap).unwrap();
        let next = b.variables(vars[0].reference, 0, 10, &heap).unwrap();
        assert_eq!(next[0].ty, "cycle");
        b.reset();
        assert!(b
            .variables(scope, 0, 10, &heap)
            .unwrap_err()
            .contains("stale"));
    }

    #[test]
    fn long_values_are_shortened_and_watch_is_read_only() {
        let heap = json!({});
        let mut b = Browser::new();
        let long = "x".repeat(500);
        let scopes = vec![vec![
            ("big".to_string(), long),
            ("xs".to_string(), "[10, 20, 30]".to_string()),
            ("p".to_string(), "Point{\"x\": Int(3)}".to_string()),
        ]];
        assert!(b
            .watch("big", &scopes, &heap)
            .unwrap()
            .value
            .contains("500 characters"));
        assert_eq!(b.watch("xs[1]", &scopes, &heap).unwrap().value, "20");
        assert_eq!(b.watch("p.x", &scopes, &heap).unwrap().value, "3");
        for bad in ["f(1)", "xs[1+1]", "p.x = 4", "await t", "xs[-1]"] {
            assert!(b.watch(bad, &scopes, &heap).is_err(), "{bad}");
        }
        assert!(b.watch("xs[9]", &scopes, &heap).is_err());
    }
}
