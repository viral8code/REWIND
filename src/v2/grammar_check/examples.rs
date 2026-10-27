//! 受理と拒否の例ファイル（段階 E）。テスト専用。
//!
//! `tests/grammar/accept/*.rw` は、実装の字句解析器と構文解析器（`Parser::program()`）も、文法（EBNF）の字句と
//! `Program` も受理しなければならない。`tests/grammar/reject/*.rw` は、両方が拒否しなければならない。
//! 拒否の理由は構文でなければならない。構文解析器が意味の検査（`NON_SYNTACTIC`）で拒否して EBNF が
//! 受理するファイルは、拒否の例として使えない。
//! ディレクトリを走査するので、ファイルを足せば自動で検査される。

use super::lexcheck::{impl_tokens, EbnfLexer};
use super::syntaxcheck::{compare, parser_verdict, Class, Entry};
use std::path::{Path, PathBuf};

fn rw_files(kind: &str) -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/grammar")
        .join(kind);
    let mut v: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "rw"))
        .collect();
    v.sort();
    v
}

/// 先頭の行が `//` のコメントで、何を確かめる例かが書いてあるか。
fn check_header(src: &str) -> Result<(), String> {
    let first = src.lines().next().unwrap_or("");
    if first.starts_with("// ") && first.chars().count() > 5 {
        Ok(())
    } else {
        Err(format!("先頭の行が「// 確かめること」でない: {first:?}"))
    }
}

fn check_accept(lx: &EbnfLexer, src: &str) -> Result<(), String> {
    check_header(src)?;
    let imp = impl_tokens(src).map_err(|e| format!("lex() が拒否: {e}"))?;
    let eb = lx
        .tokenize(src)
        .map_err(|e| format!("EBNF の字句が拒否: {e:?}"))?;
    if imp != eb {
        return Err("lex() と EBNF の字句でトークン列が違う".into());
    }
    let r = compare(Entry::Program, &imp);
    if !r.parser_ok {
        let e = parser_verdict(Entry::Program, &imp)
            .0
            .err()
            .unwrap_or_default();
        return Err(format!(
            "Parser が拒否: {}",
            e.chars().take(120).collect::<String>()
        ));
    }
    if !r.earley_ok {
        return Err("EBNF の Program が拒否".into());
    }
    Ok(())
}

fn check_reject(lx: &EbnfLexer, src: &str) -> Result<(), String> {
    check_header(src)?;
    match (impl_tokens(src), lx.tokenize(src)) {
        // 字句の誤り。両方が拒否すること。
        (Err(_), Err(_)) => Ok(()),
        (Err(e), Ok(_)) => Err(format!("lex() だけが拒否（既知の差の可能性）: {e}")),
        (Ok(_), Err(e)) => Err(format!("EBNF の字句だけが拒否: {e:?}")),
        (Ok(t), Ok(eb)) => {
            if t != eb {
                return Err("lex() と EBNF の字句でトークン列が違う".into());
            }
            let r = compare(Entry::Program, &t);
            match r.class {
                Class::Agree if !r.parser_ok => Ok(()),
                Class::Agree => Err("Parser も EBNF も受理した".into()),
                Class::NonSyntactic => Err("Parser の拒否は意味の検査で、EBNF は受理した".into()),
                Class::KnownGap(g) => Err(format!("既知の差: {g}")),
                Class::Mismatch(m) => Err(m),
            }
        }
    }
}

#[test]
fn grammar_example_files() {
    let lx = EbnfLexer::new();
    let accept = rw_files("accept");
    let reject = rw_files("reject");
    assert!(
        accept.len() >= 30,
        "accept は 30 件以上（{}）",
        accept.len()
    );
    assert!(
        reject.len() >= 30,
        "reject は 30 件以上（{}）",
        reject.len()
    );
    let mut bad = Vec::new();
    for (files, kind) in [(&accept, "accept"), (&reject, "reject")] {
        for p in files {
            let name = p.file_name().unwrap().to_string_lossy().to_string();
            let Ok(src) = std::fs::read_to_string(p) else {
                bad.push(format!("{kind}/{name}: 読めない"));
                continue;
            };
            let r = if kind == "accept" {
                check_accept(&lx, &src)
            } else {
                check_reject(&lx, &src)
            };
            if let Err(m) = r {
                bad.push(format!("{kind}/{name}: {m}"));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "例ファイルの食い違い（{} 件）:\n{}",
        bad.len(),
        bad.join("\n")
    );
}
