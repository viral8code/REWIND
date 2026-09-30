use super::*;
pub(super) mod diagnostics;
mod effects;
mod library;
pub(super) use effects::{closure_effects, function_effects, infer, validate};
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
    if let Some(p) = expected.iter().find(|p| params.contains(p)) {
        let value = format!("@{}", actual.into_iter().collect::<Vec<_>>().join("+"));
        return match map.get(p) {
            Some(old) => old == &value,
            None => {
                map.insert(p.clone(), value);
                true
            }
        };
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
