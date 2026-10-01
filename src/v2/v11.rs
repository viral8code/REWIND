//! Developer diagnostics; no values or captured environments enter a backtrace.
use super::*;

pub(super) fn display_symbol(name: &str) -> String {
    if name.starts_with("$closure") {
        return "<closure>".into();
    }
    name.strip_prefix("$import$")
        .or_else(|| name.strip_prefix("$impl$"))
        .unwrap_or(name)
        .replace('$', "::")
}

pub(super) fn enrich(d: &mut rewind::DiagnosticRecord) {
    let m = d.message.as_str();
    let (code, hint) = if m == "division by zero" {
        (
            "DivisionByZero",
            "Check the divisor before division or remainder.",
        )
    } else if m == "integer overflow" || m == "integer literal overflow" {
        (
            "IntegerOverflow",
            "Int is signed 64-bit. Check the operands or use std.integer checked operations.",
        )
    } else if m.contains("index out of bounds") {
        (
            "IndexOutOfBounds",
            "Indices start at zero. Check the index against len() before accessing the collection.",
        )
    } else if m.starts_with("assertion failed") || m.starts_with("assert_eq failed") {
        (
            "AssertionFailed",
            "Check the assertion and its inputs; pending output is only visible after publish.",
        )
    } else if m.starts_with("panic:") {
        ("Panic", "This failure was explicitly raised by panic. Inspect the failure location and callers.")
    } else if m.starts_with("unknown variable ")
        || m.starts_with("unknown function ")
        || m.starts_with("unknown name ")
    {
        (
            "UnknownName",
            "Check the spelling, binding scope and import alias.",
        )
    } else if m.contains("cannot use moved")
        || m.contains("use after move")
        || m.contains("was moved")
    {
        ("UseAfterMove", "Borrow with & or &mut when ownership must remain with the caller; use freeze for a shared snapshot.")
    } else if m.contains("private field") {
        (
            "PrivateField",
            "Use a public constructor or accessor provided by the defining module.",
        )
    } else {
        (d.code.as_str(), "")
    };
    let code = code.to_string();
    let hint = if !hint.is_empty() {
        hint
    } else {
        match code.as_str() {
            "GuiUnavailable" => "Native GUI needs an interactive Win32 desktop or an X11 display with libX11/core fonts. Use --gui-events FILE for headless tests.",
            "GuiNotPublished" => "Call std.gui.present and publish before reading a GUI event.",
            "GuiMainTaskOnly" => "Read GUI input in the application event loop; run background computation as tasks.",
            "InvalidArguments" => "Match the expected argument count and types; Result<T,E> is different from T.",
            "InvalidContinuation" => "Revert requires the same live call frames and checkpoint scopes. Use resume to restore the saved execution position, or commit in a scope that is still active.",
            "InvalidNumericLiteral" => "Use base digits after 0x/0b/0o and place underscores only between digits. Exponents need decimal digits.",
            "UnterminatedComment" => "Close every /* comment with */; block comments may be nested.",
            "InvalidUnicodeEscape" => "Use \\u{HEX} with 1 to 6 hex digits for a valid Unicode scalar; surrogate code points are invalid.",
            "ExecutionBudgetExceeded" => "Check for a non-terminating loop; increase --steps only when the work is intentional.",
            "NativeWorkBudgetExceeded" => "Reduce input or collection work, or raise --native-work for intentional work.",
            "TaskSteps" => "Check the task loop or increase --task-steps for intentional work.",
            "HistoryMemory" | "HistoryStorage" => "Drop checkpoints you no longer need and reduce retained data; publish alone does not drop checkpoints.",
            "PublishPartiallyApplied" => "Some external effects may already be applied. Inspect publish_failure; do not blindly retry.",
            "ExternalStateConflict" => "An external file changed. Reload its state and decide how to reconcile it before publishing.",
            "TaskDeadlock" => "Inspect the wait graph; make sure each awaited task or channel can make progress.",
            "EffectMissing" | "MissingEffect" => "Declare the effect in the function and grant the corresponding CLI or manifest permission.",
            "NotFound" => "File paths are relative to the execution root; check the path and its spelling.",
            _ => "",
        }
    };
    d.code = code;
    if !hint.is_empty() && d.hints.is_empty() {
        d.hints.push(hint.into());
    }
}

/// Terminal escapes/control bytes are never interpreted in diagnostics.
fn safe(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_control() && c != '\n' && c != '\t' {
                '\u{fffd}'
            } else {
                c
            }
        })
        .collect()
}
pub(super) fn render(error: &Error) -> String {
    let Error::Diagnostic(d) = error else {
        return format!("rewind: {}", safe(&error.to_string()));
    };
    fn record(d: &rewind::DiagnosticRecord, out: &mut String, depth: usize) {
        use std::fmt::Write;
        if depth >= 8 {
            let _ = writeln!(out, "  further causes omitted");
            return;
        }
        let prefix = if depth == 0 { "error" } else { "caused by" };
        let _ = writeln!(out, "{prefix}[{}]: {}", safe(&d.code), safe(&d.message));
        if d.line != 0 {
            let _ = writeln!(out, "  --> {}:{}:{}", safe(&d.source), d.line, d.column);
        }
        for frame in &d.frames {
            let _ = writeln!(
                out,
                "  at {} ({}:{}:{})",
                safe(&frame.function),
                safe(&frame.source),
                frame.line,
                frame.column
            );
        }
        if d.frames_truncated {
            let _ = writeln!(out, "  further callers omitted");
        }
        for hint in &d.hints {
            let _ = writeln!(out, "  help: {}", safe(hint));
        }
        for cause in &d.causes {
            record(cause, out, depth + 1);
        }
    }
    let mut out = String::new();
    record(d, &mut out, 0);
    out.trim_end().into()
}

/// A bounded, deterministic spelling suggestion using visible lexical bindings.
pub(super) fn unknown_name(at: &Tok, name: &str, names: impl Iterator<Item = String>) -> Error {
    let mut error = diagnostic(at, format!("unknown name {name}"));
    let Error::Diagnostic(d) = &mut error else {
        return error;
    };
    enrich(d);
    if name.chars().count() > 64 {
        return error;
    }
    fn distance(a: &str, b: &str) -> usize {
        let a = a.chars().collect::<Vec<_>>();
        let b = b.chars().collect::<Vec<_>>();
        let mut row = (0..=b.len()).collect::<Vec<_>>();
        for (i, x) in a.iter().enumerate() {
            let mut diagonal = row[0];
            row[0] = i + 1;
            for (j, y) in b.iter().enumerate() {
                let old = row[j + 1];
                row[j + 1] = (row[j] + 1)
                    .min(old + 1)
                    .min(diagonal + usize::from(x != y));
                diagonal = old;
            }
        }
        row[b.len()]
    }
    let threshold = if name.chars().count() < 5 { 1 } else { 2 };
    let best = names
        .take(256)
        .filter(|s| s.chars().count() <= 64)
        .map(|s| (distance(name, &s), s))
        .filter(|(n, _)| *n <= threshold)
        .min();
    if let Some((_, suggestion)) = best {
        d.hints.insert(0, format!("Did you mean `{suggestion}`?"));
    }
    error
}
