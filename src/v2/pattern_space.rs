//! Bounded constructor-matrix coverage for nested patterns.
use super::*;
#[derive(Clone, Debug, PartialEq, Eq)]
enum Node {
    Wild,
    Ctor(String, Vec<Node>),
    Int(i128, i128),
    Text(String),
    List(Vec<Node>),
}
#[derive(Clone)]
struct Ctor {
    tag: String,
    fields: Vec<(String, String)>,
    interval: Option<(i128, i128)>,
}
fn arguments(ty: &str) -> Vec<&str> {
    ty.split_once('<')
        .map(|(_, v)| split_type_args(outer_type_end(v)))
        .unwrap_or_default()
}
fn substitute(params: &[String], ty: &str) -> BTreeMap<String, String> {
    params
        .iter()
        .cloned()
        .zip(arguments(ty).into_iter().map(str::to_string))
        .collect()
}
fn finite(p: &Program, ty: &str) -> Option<Vec<Ctor>> {
    let base = ty.split('<').next().unwrap_or(ty);
    let make = |tag: &str, fields: Vec<String>| Ctor {
        tag: tag.into(),
        fields: fields
            .into_iter()
            .enumerate()
            .map(|(i, t)| (i.to_string(), t))
            .collect(),
        interval: None,
    };
    if ty == "Bool" {
        return Some(vec![make("true", vec![]), make("false", vec![])]);
    }
    if base == "Option" {
        return Some(vec![
            make("None", vec![]),
            make(
                "Some",
                arguments(ty).into_iter().map(str::to_string).collect(),
            ),
        ]);
    }
    if base == "Result" {
        let a = arguments(ty);
        return Some(vec![
            make("Ok", vec![a[0].into()]),
            make("Err", vec![a[1].into()]),
        ]);
    }
    if base == "Tuple" {
        return Some(vec![make(
            "$tuple",
            arguments(ty).into_iter().map(str::to_string).collect(),
        )]);
    }
    if base == "FileError" {
        return Some(vec![Ctor {
            tag: "$record".into(),
            fields: vec![
                ("code".into(), "String".into()),
                ("path".into(), "String".into()),
                ("cause".into(), "Option<String>".into()),
                ("causes".into(), "List<String>".into()),
            ],
            interval: None,
        }]);
    }
    if let Some(def) = p.enums.get(base) {
        let map = substitute(&def.type_params, ty);
        return Some(
            def.variants
                .iter()
                .map(|(name, fields)| Ctor {
                    tag: name.clone(),
                    fields: fields
                        .iter()
                        .map(|(n, t)| (n.clone(), substitute_type(t, &map)))
                        .collect(),
                    interval: None,
                })
                .collect(),
        );
    }
    if let Some(def) = p.structs.get(base) {
        if def.private_fields.contains("$native") {
            return None;
        }
        let map = substitute(&def.type_params, ty);
        return Some(vec![Ctor {
            tag: "$record".into(),
            fields: def
                .fields
                .iter()
                .map(|(n, t)| (n.clone(), substitute_type(t, &map)))
                .collect(),
            interval: None,
        }]);
    }
    None
}
fn normalize(p: &Program, pat: &Pattern, ty: &str) -> Node {
    let pat = resolve_pattern_alias(pat, &p.import_aliases);
    match pat {
        Pattern::Wildcard | Pattern::Bind(_) => Node::Wild,
        Pattern::Literal(Value::Bool(v)) => Node::Ctor(v.to_string(), vec![]),
        Pattern::Literal(Value::Int(v)) => Node::Int(v as i128, v as i128 + 1),
        Pattern::Literal(Value::Text(v)) => Node::Text(v.to_string()),
        Pattern::Range(a, b) => Node::Int(a as i128, b as i128),
        Pattern::List(parts) => {
            let inner = arguments(ty).first().copied().unwrap_or("Unknown");
            Node::List(parts.iter().map(|v| normalize(p, v, inner)).collect())
        }
        Pattern::Variant(name, parts) => {
            let tag = name.rsplit("::").next().unwrap();
            let c = finite(p, ty)
                .unwrap_or_default()
                .into_iter()
                .find(|c| c.tag == tag);
            Node::Ctor(
                tag.into(),
                parts
                    .iter()
                    .enumerate()
                    .map(|(i, part)| {
                        normalize(
                            p,
                            part,
                            c.as_ref()
                                .and_then(|c| c.fields.get(i))
                                .map_or("Unknown", |(_, t)| t),
                        )
                    })
                    .collect(),
            )
        }
        Pattern::Struct(name, parts) => {
            let tag = if name.contains("::") {
                name.rsplit("::").next().unwrap()
            } else {
                "$record"
            };
            let c = finite(p, ty)
                .unwrap_or_default()
                .into_iter()
                .find(|c| c.tag == tag);
            Node::Ctor(
                tag.into(),
                c.map_or_else(Vec::new, |c| {
                    c.fields
                        .iter()
                        .map(|(n, t)| {
                            parts
                                .iter()
                                .find(|(name, _)| name == n)
                                .map_or(Node::Wild, |(_, part)| normalize(p, part, t))
                        })
                        .collect()
                }),
            )
        }
        _ => Node::Wild,
    }
}
fn specialization(n: &Node, c: &Ctor) -> Option<Vec<Node>> {
    match n {
        Node::Wild => Some(vec![Node::Wild; c.fields.len()]),
        Node::Ctor(tag, parts) if tag == &c.tag => Some(parts.clone()),
        Node::Text(s) if c.tag == format!("$text:{s}") => Some(vec![]),
        Node::Int(a, b) if c.interval.is_some_and(|(lo, hi)| *a <= lo && *b >= hi) => Some(vec![]),
        Node::List(parts) if c.tag == format!("$list:{}", parts.len()) => Some(parts.clone()),
        _ => None,
    }
}
struct Analysis<'a> {
    program: &'a Program,
    remaining: usize,
}
impl Analysis<'_> {
    fn useful(
        &mut self,
        matrix: &[Vec<Node>],
        query: &[Node],
        types: &[String],
        depth: usize,
    ) -> std::result::Result<bool, ()> {
        let work = 1usize
            .saturating_add(query.len())
            .saturating_add(matrix.iter().map(Vec::len).sum::<usize>());
        self.remaining = self.remaining.checked_sub(work).ok_or(())?;
        if depth > 256 {
            return Err(());
        }
        if query.is_empty() {
            return Ok(matrix.is_empty());
        }
        if matrix
            .iter()
            .any(|row| row == query || row.iter().all(|n| matches!(n, Node::Wild)))
        {
            return Ok(false);
        }
        // Empty ranges remain impossible inside constructor and list products.
        let mut nodes = query.iter().collect::<Vec<_>>();
        while let Some(node) = nodes.pop() {
            self.remaining = self.remaining.checked_sub(1).ok_or(())?;
            match node {
                Node::Int(a, b) if a >= b => return Ok(false),
                Node::Ctor(_, parts) | Node::List(parts) => nodes.extend(parts),
                _ => {}
            }
        }
        if matrix.is_empty() {
            return Ok(!types
                .iter()
                .any(|t| finite(self.program, t).is_some_and(|c| c.is_empty())));
        }
        let skip = query
            .iter()
            .enumerate()
            .take_while(|(i, n)| {
                matches!(n, Node::Wild)
                    && matrix.iter().all(|r| matches!(r.get(*i), Some(Node::Wild)))
            })
            .count();
        if skip > 0 {
            return self.useful(
                &matrix
                    .iter()
                    .map(|r| r[skip..].to_vec())
                    .collect::<Vec<_>>(),
                &query[skip..],
                &types[skip..],
                depth + 1,
            );
        }
        let ty = &types[0];
        let q = &query[0];
        let ctors = if ty == "Int" {
            let mut cuts = BTreeSet::from([i64::MIN as i128, i64::MAX as i128 + 1]);
            for n in matrix
                .iter()
                .filter_map(|r| r.first())
                .chain(std::iter::once(q))
            {
                if let Node::Int(a, b) = n {
                    cuts.insert(*a);
                    cuts.insert(*b);
                }
            }
            let cuts = cuts.into_iter().collect::<Vec<_>>();
            Some(
                cuts.windows(2)
                    .filter(|v| v[0] < v[1])
                    .map(|v| Ctor {
                        tag: "$int".into(),
                        fields: vec![],
                        interval: Some((v[0], v[1])),
                    })
                    .collect::<Vec<_>>(),
            )
        } else {
            finite(self.program, ty)
        };
        let ctors = match ctors {
            Some(c) => c,
            None => match q {
                Node::Wild => {
                    let defaults = matrix
                        .iter()
                        .filter(|r| matches!(r.first(), Some(Node::Wild)))
                        .map(|r| r[1..].to_vec())
                        .collect::<Vec<_>>();
                    return self.useful(&defaults, &query[1..], &types[1..], depth + 1);
                }
                Node::Text(s) => vec![Ctor {
                    tag: format!("$text:{s}"),
                    fields: vec![],
                    interval: None,
                }],
                Node::List(parts) => {
                    let args = arguments(ty);
                    let inner = args.first().copied().unwrap_or("Unknown");
                    vec![Ctor {
                        tag: format!("$list:{}", parts.len()),
                        fields: (0..parts.len())
                            .map(|i| (i.to_string(), inner.into()))
                            .collect(),
                        interval: None,
                    }]
                }
                _ => return Ok(true),
            },
        };
        for c in ctors {
            let Some(mut specialized_query) = specialization(q, &c) else {
                continue;
            };
            specialized_query.extend_from_slice(&query[1..]);
            let specialized_matrix = matrix
                .iter()
                .filter_map(|row| {
                    let mut out = specialization(&row[0], &c)?;
                    out.extend_from_slice(&row[1..]);
                    Some(out)
                })
                .collect::<Vec<_>>();
            let mut next_types = c.fields.iter().map(|(_, t)| t.clone()).collect::<Vec<_>>();
            next_types.extend_from_slice(&types[1..]);
            if self.useful(
                &specialized_matrix,
                &specialized_query,
                &next_types,
                depth + 1,
            )? {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
pub(super) fn check(p: &Program, ty: &str, patterns: &[(&Pattern, bool)], at: &Tok) -> Result<()> {
    let mut analysis = Analysis {
        program: p,
        remaining: 250_000,
    };
    let mut matrix = vec![];
    let types = vec![ty.to_string()];
    for (pat, guarded) in patterns {
        let row = vec![normalize(p, pat, ty)];
        if !analysis
            .useful(&matrix, &row, &types, 0)
            .map_err(|_| diagnostic(at, "PatternAnalysisBudgetExceeded"))?
        {
            return Err(diagnostic(at, "unreachable match arm"));
        }
        if !guarded {
            matrix.push(row);
        }
    }
    if analysis
        .useful(&matrix, &[Node::Wild], &types, 0)
        .map_err(|_| diagnostic(at, "PatternAnalysisBudgetExceeded"))?
    {
        return Err(diagnostic(at, format!("non-exhaustive match for {ty}")));
    }
    Ok(())
}
