//! DOC-01: every public standard-library function states its cost. Functions that predate the rule are listed in
//! `libraries/api/undocumented-cost.txt`; the list may only shrink. A new function without a cost, or a listed
//! function that gained one (so its line must be removed), fails here.
use std::collections::BTreeSet;
use std::fs;

fn undocumented_now() -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for entry in fs::read_dir("libraries/api").unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let Some(module) = name.strip_suffix(".api.json") else {
            continue;
        };
        let document: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        for (symbol, value) in document["symbols"].as_object().unwrap() {
            let Some(function) = symbol.strip_prefix("fn:") else {
                continue;
            };
            if function.starts_with("std") {
                continue;
            }
            let cost = value["cost"].as_str().unwrap_or("");
            if cost.trim().is_empty() {
                found.insert(format!("{module}:{function}"));
            }
        }
    }
    found
}

#[test]
fn new_public_functions_state_their_cost_and_the_exception_list_only_shrinks() {
    let listed: BTreeSet<String> = fs::read_to_string("libraries/api/undocumented-cost.txt")
        .unwrap()
        .lines()
        .map(str::to_owned)
        .filter(|line| !line.is_empty())
        .collect();
    let now = undocumented_now();
    let new_gaps: Vec<_> = now.difference(&listed).collect();
    assert!(
        new_gaps.is_empty(),
        "public functions without a `// @api <name> cost ...` line: {new_gaps:?}"
    );
    let fixed: Vec<_> = listed.difference(&now).collect();
    assert!(
        fixed.is_empty(),
        "these functions now state a cost; remove them from libraries/api/undocumented-cost.txt: {fixed:?}"
    );
}

#[test]
fn cost_statements_are_not_placeholders() {
    for entry in fs::read_dir("libraries/api").unwrap() {
        let path = entry.unwrap().path();
        if !path.to_string_lossy().ends_with(".api.json") {
            continue;
        }
        let document: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        for (symbol, value) in document["symbols"].as_object().unwrap() {
            if let Some(cost) = value["cost"].as_str() {
                assert!(
                    cost.trim().len() >= 3,
                    "{}: {symbol} has an empty-looking cost {cost:?}",
                    path.display()
                );
            }
        }
    }
}

#[test]
fn every_new_standard_module_has_an_api_baseline() {
    let exempt: BTreeSet<String> = fs::read_to_string("libraries/api/no-baseline.txt")
        .unwrap()
        .lines()
        .map(str::to_owned)
        .filter(|line| !line.is_empty())
        .collect();
    let mut missing = BTreeSet::new();
    for entry in fs::read_dir("libraries/std").unwrap() {
        let path = entry.unwrap().path();
        let Some(module) = path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.strip_suffix(".rw"))
        else {
            continue;
        };
        if module == "tests" || !fs::read_to_string(&path).unwrap().contains("pub fn") {
            continue;
        }
        if !std::path::Path::new(&format!("libraries/api/{module}.api.json")).exists() {
            missing.insert(module.to_owned());
        }
    }
    let new: Vec<_> = missing.difference(&exempt).collect();
    assert!(
        new.is_empty(),
        "standard modules without libraries/api/<name>.api.json (generate it with sdk-build): {new:?}"
    );
    let stale: Vec<_> = exempt.difference(&missing).collect();
    assert!(
        stale.is_empty(),
        "these modules now have a baseline; remove them from libraries/api/no-baseline.txt: {stale:?}"
    );
}
