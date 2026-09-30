use super::*;
use std::io::{Read, Write};

pub(in crate::v2) fn load(
    root: &Path,
    config: &project::ProjectConfig,
    source: &str,
) -> Result<Program> {
    let path = config.source_root.join(".rewind-repl.rw");
    let overlay = BTreeMap::from([(path.clone(), source.to_string())]);
    let mut program = load_program_overlay(
        &path,
        root,
        &config.imports,
        &mut BTreeSet::new(),
        &mut BTreeMap::new(),
        &overlay,
    )?;
    program.language = config.language.clone();
    program.strict_visibility = true;
    v05::prepare(&mut program)?;
    check_program(&program)?;
    v05::validate(&program, config)?;
    fn restricted(body: &[Stmt]) -> bool {
        body.iter().any(|s| match &s.kind {
            StmtKind::Runtime(_, _) | StmtKind::Defer(_) | StmtKind::Using(_, _) => true,
            StmtKind::Block(b)
            | StmtKind::While(_, b)
            | StmtKind::For(_, _, _, b)
            | StmtKind::Branch(_, b) => restricted(b),
            StmtKind::If(_, a, b) => restricted(a) || restricted(b),
            StmtKind::Match(_, arms) => arms
                .iter()
                .any(|(_, _, s)| restricted(std::slice::from_ref(s))),
            _ => false,
        })
    }
    let mut forbidden = restricted(&program.stmts)
        || program
            .functions
            .values()
            .any(|f| f.asynchronous || restricted(&f.body));
    v05::expressions(&program.stmts, &mut |e| {
        if let ExprKind::Closure(_, _, b) = &e.kind {
            forbidden |= restricted(b);
        }
    });
    for f in program.functions.values() {
        v05::expressions(&f.body, &mut |e| {
            if let ExprKind::Closure(_, _, b) = &e.kind {
                forbidden |= restricted(b);
            }
        });
    }
    if forbidden || program.module_effects.values().any(|e| e.contains("tasks")) {
        return Err(Error::InvalidOperation(
            "REPL does not support tasks, cleanup or runtime budget overrides".into(),
        ));
    }
    Ok(program)
}

pub(in crate::v2) fn repl(root: &Path) -> Result<()> {
    let config = project::ProjectConfig::load(root)?
        .ok_or_else(|| Error::InvalidOperation("repl requires a manifest".into()))?;
    if config.language != "0.8" {
        return Err(Error::InvalidOperation("repl requires language 0.8".into()));
    }
    config.lock(root, false)?;
    let root = fs::canonicalize(root)?;
    let mut accepted = String::new();
    let mut previous: Option<Program> = None;
    let mut observations = None;
    let mut state = json!({});
    let stdin = io::stdin();
    let mut input = io::BufReader::new(stdin.lock().take(1024 * 1024 + 1));
    let stdout = io::stdout();
    let mut output = stdout.lock();
    for index in 0..256 {
        let mut line = String::new();
        if input.read_line(&mut line)? == 0 {
            return Ok(());
        }
        if line.len() > 64 * 1024 {
            return Err(Error::InvalidOperation("REPL input exceeds 64 KiB".into()));
        }
        match line.trim() {
            "" => continue,
            ":quit" => return Ok(()),
            ":state" => {
                writeln!(output, "{}", json!({"state":state}))?;
                output.flush()?;
                continue;
            }
            ":reset" => {
                accepted.clear();
                previous = None;
                observations = None;
                state = json!({});
                writeln!(output, "{}", json!({"ok":true,"reset":true}))?;
                output.flush()?;
                continue;
            }
            _ => {}
        }
        let candidate = format!("{accepted}{line}");
        let result = (|| {
            if candidate.len() > 1024 * 1024 {
                return Err(Error::InvalidOperation("REPL source exceeds 1 MiB".into()));
            }
            let program = load(&root, &config, &candidate)?;
            // Declarations after execution may change earlier dispatch/effects. Keep the prefix immutable.
            if let Some(old) = previous.as_ref().filter(|p| !p.stmts.is_empty()) {
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
            let (outcome, tape, view) = vm::session(program.clone(), &root, observations.as_ref())?;
            // Failed computations are discarded; their observations remain available for a retry.
            observations = Some(tape);
            outcome?;
            accepted = candidate;
            previous = Some(program);
            state = view;
            Ok(())
        })();
        let response = match result {
            Ok(()) => json!({"input":index,"ok":true,"state":state}),
            Err(e) => {
                json!({"input":index,"ok":false,"error":e.to_string(),"diagnostic":if let Error::Diagnostic(d)=&e {Some(d.as_ref())} else {None},"state":state})
            }
        };
        writeln!(output, "{response}")?;
        output.flush()?;
    }
    Err(Error::InvalidOperation(
        "REPL input budget exceeded (256 lines)".into(),
    ))
}
