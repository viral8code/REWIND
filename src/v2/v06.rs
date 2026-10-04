use super::*;
pub(super) mod cache;
pub(super) mod captures;
pub(super) mod debug;
pub(super) mod diagnostics;
mod effects;
pub(super) mod language;
mod library;
pub(super) mod resolver;
pub(super) mod update;
pub(super) use self::effects::{closure_effects, entry_effects, function_effects, infer, validate};
pub(super) const KNOWN: &[&str] = &[
    "fileRead",
    "fileWrite",
    "output",
    "input",
    "clock",
    "random",
    "env",
    "args",
    "locale",
    "tasks",
    "gui",
    "external",
    "live",
    "network",
    "db",
];
pub(super) fn borrowed_type(t: &str) -> &str {
    t.strip_prefix("&mut ")
        .or_else(|| t.strip_prefix('&'))
        .unwrap_or(t)
}
pub(super) fn strip_effects(t: &str) -> &str {
    let mut nesting = 0i32;
    let mut cut = None;
    for (i, c) in t.char_indices() {
        match c {
            '(' | '<' => nesting += 1,
            ')' | '>' => {
                if i > 0 && t.as_bytes()[i - 1] == b'-' {
                    continue;
                }
                nesting -= 1;
            }
            '!' if nesting == 0 => cut = Some(i),
            _ => {}
        }
    }
    cut.map(|i| &t[..i]).unwrap_or(t)
}
pub(super) fn type_effects(t: &str) -> BTreeSet<String> {
    let base = strip_effects(t);
    let suffix = &t[base.len()..];
    let suffix = suffix.split("~{").next().unwrap_or(suffix);
    suffix
        .strip_prefix("!{")
        .and_then(|s| s.strip_suffix('}'))
        .map(|s| {
            s.split('+')
                .map(|s| s.trim_start_matches('@'))
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_else(|| KNOWN.iter().map(|s| s.to_string()).collect())
}
pub(super) fn effects_compatible(expected: &str, actual: &str) -> bool {
    let e = type_effects(expected);
    let a = type_effects(actual);
    a.is_subset(&e)
        || e.iter().any(|s| !KNOWN.contains(&s.as_str()))
        || strip_effects(actual) == actual
}
pub(super) fn unify_effects(
    e: &str,
    a: &str,
    params: &[String],
    map: &mut BTreeMap<String, String>,
) -> bool {
    let expected = type_effects(e);
    let actual = type_effects(a);
    let variables = expected
        .iter()
        .filter(|p| params.contains(p))
        .collect::<Vec<_>>();
    if !variables.is_empty() {
        for p in &variables {
            map.entry((*p).clone()).or_insert_with(|| "@".into());
        }
        if variables.len() == 1 {
            let p = variables[0];
            let mut effects = map[p]
                .trim_start_matches('@')
                .split('+')
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect::<BTreeSet<_>>();
            effects.extend(actual.difference(&expected).cloned());
            map.insert(
                p.clone(),
                format!("@{}", effects.into_iter().collect::<Vec<_>>().join("+")),
            );
        }
        return true;
    }
    effects_compatible(e, a)
}
pub(super) fn fn_type(args: &[(String, String)], ret: &str, effects: &BTreeSet<String>) -> String {
    format!(
        "fn({})->{ret}!{{{}}}",
        args.iter()
            .map(|(_, t)| t.as_str())
            .collect::<Vec<_>>()
            .join(","),
        effects.iter().cloned().collect::<Vec<_>>().join("+")
    )
}
pub(super) fn checker_method(
    checker: &Checker<'_>,
    ty: &str,
    m: &str,
    args: &[String],
    at: &Tok,
) -> Result<Option<String>> {
    library::method_type(checker, ty, m, args, at)
}
pub(super) use library::primitive_method;
pub(super) fn immutable_tuple(value: Value, rt: &Runtime) -> Value {
    match value {
        Value::HeapRef(id) => match rt.heap_get(id) {
            Some(Value::Struct(t, _)) if t.starts_with("Tuple<") => {
                immutable_tuple(rt.heap_get(id).unwrap().clone(), rt)
            }
            _ => Value::HeapRef(id),
        },
        Value::Struct(t, fields) if t.starts_with("Tuple<") => Value::Struct(
            t,
            fields
                .into_iter()
                .map(|(n, v)| (n, immutable_tuple(v, rt)))
                .collect(),
        ),
        Value::Option(v) => Value::Option(v.map(|v| Box::new(immutable_tuple(*v, rt)))),
        Value::Result(Ok(v)) => Value::Result(Ok(Box::new(immutable_tuple(*v, rt)))),
        Value::Result(Err(v)) => Value::Result(Err(Box::new(immutable_tuple(*v, rt)))),
        v => v,
    }
}

pub(super) fn solve_effects(
    expected: &str,
    actual: &str,
    params: &[String],
    map: &mut BTreeMap<String, String>,
) -> bool {
    let expected = borrowed_type(expected);
    let actual = borrowed_type(actual);
    if let (Some((ea, er)), Some((aa, ar))) =
        (function_signature(expected), function_signature(actual))
    {
        for (e, a) in ea.iter().zip(aa) {
            if !solve_effects(e, &a, params, map) {
                return false;
            }
        }
        if !solve_effects(&er, &ar, params, map) {
            return false;
        }
        if strip_effects(actual) == actual {
            return true;
        }
        let needs = type_effects(actual);
        let upper = type_effects(expected);
        let mut allowed = upper
            .iter()
            .filter(|e| !params.contains(e))
            .cloned()
            .collect::<BTreeSet<_>>();
        let vars = upper
            .iter()
            .filter(|e| params.contains(e))
            .collect::<Vec<_>>();
        for v in &vars {
            allowed.extend(
                map.get(*v)
                    .into_iter()
                    .flat_map(|s| s.trim_start_matches('@').split('+'))
                    .filter(|s| !s.is_empty())
                    .map(str::to_string),
            );
        }
        let missing = needs.difference(&allowed).cloned().collect::<BTreeSet<_>>();
        if !missing.is_empty() {
            let Some(v) = vars.first() else {
                return false;
            };
            let mut effects = map[*v]
                .trim_start_matches('@')
                .split('+')
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect::<BTreeSet<_>>();
            effects.extend(missing);
            map.insert(
                (*v).clone(),
                format!("@{}", effects.into_iter().collect::<Vec<_>>().join("+")),
            );
        }
    } else if let (Some((_, ei)), Some((_, ai))) =
        (expected.split_once('<'), actual.split_once('<'))
    {
        for (e, a) in split_type_args(outer_type_end(ei))
            .into_iter()
            .zip(split_type_args(outer_type_end(ai)))
        {
            if !solve_effects(e, a, params, map) {
                return false;
            }
        }
    }
    true
}
