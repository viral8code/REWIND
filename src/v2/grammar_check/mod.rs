//! 形式文法（EBNF）を検証する仕組みの基盤（段階 A）。テスト専用。
//!
//! - `ebnf`: W3C 形式の EBNF の読み込み、層の検査、BNF への展開。
//! - `earley`: Earley 認識器（認識だけ）。
//!
//! 玩具の文法で動作を確かめる単体テストのほか、`lexcheck`（字句）と `syntaxcheck`（型・パターン・式）が
//! `docs/grammar/rewind.ebnf` を実装と突き合わせる。
#![allow(dead_code)]

pub mod earley;
pub mod ebnf;
#[cfg(test)]
pub mod examples;
#[cfg(test)]
pub mod lexcheck;
#[cfg(test)]
pub mod syntaxcheck;

#[cfg(test)]
mod tests {
    use super::earley::{Earley, RecognizeError};
    use super::ebnf::{Grammar, Token};
    use std::time::Instant;

    struct Tok {
        kind: &'static str,
        text: String,
    }

    impl Token for Tok {
        fn text(&self) -> &str {
            &self.text
        }
        fn kind(&self) -> &str {
            self.kind
        }
    }

    /// 数と 1 文字の記号に分ける。数のトークンの種別は NUMBER。
    fn lex_arith(s: &str) -> Vec<Tok> {
        let cs: Vec<char> = s.chars().collect();
        let mut out = Vec::new();
        let mut i = 0;
        while i < cs.len() {
            if cs[i].is_whitespace() {
                i += 1;
            } else if cs[i].is_ascii_digit() {
                let st = i;
                while i < cs.len() && cs[i].is_ascii_digit() {
                    i += 1;
                }
                out.push(Tok {
                    kind: "NUMBER",
                    text: cs[st..i].iter().collect(),
                });
            } else {
                out.push(Tok {
                    kind: "",
                    text: cs[i].to_string(),
                });
                i += 1;
            }
        }
        out
    }

    /// 空白で区切った語をそのままトークンにする。
    fn words(s: &str) -> Vec<&str> {
        s.split_whitespace().collect()
    }

    /// 1 文字ずつのトークンにする。
    fn chars(s: &str) -> Vec<&str> {
        s.char_indices()
            .map(|(i, c)| &s[i..i + c.len_utf8()])
            .collect()
    }

    fn run<T: Token>(src: &str, start: &str, toks: &[T]) -> Result<(), RecognizeError> {
        let g = Grammar::parse(src, &[start]).unwrap_or_else(|e| panic!("読み込み: {e:?}"));
        let bnf = g.compile(start).unwrap_or_else(|e| panic!("展開: {e:?}"));
        Earley::new(&bnf).recognize(toks).map(|_| ())
    }

    fn ok<T: Token>(src: &str, start: &str, toks: &[T]) -> bool {
        run(src, start, toks).is_ok()
    }

    fn load_errors(src: &str, roots: &[&str]) -> Vec<String> {
        match Grammar::parse(src, roots) {
            Ok(_) => Vec::new(),
            Err(e) => e,
        }
    }

    const ARITH: &str = "
        /* 算術式 */
        Expr   ::= Expr ('+' | '-') Term | Term
        Term   ::= Term ('*' | '/') Factor | Factor
        Factor ::= NUMBER | '(' Expr ')' | '-' Factor
        NUMBER ::= [0-9]+
    ";

    #[test]
    fn arithmetic_precedence_and_parens() {
        for s in [
            "1",
            "1+2",
            "1+2*3",
            "(1+2)*3",
            "((4))",
            "1 - 2 - 3",
            "2*(3+4)*5",
            "-1+-2",
            "12/(3-1)",
        ] {
            assert!(ok(ARITH, "Expr", &lex_arith(s)), "受理すべき: {s}");
        }
        for s in [
            "", "1+", "+1", "(1", "1)", "()", "1 2", "1**2", "(1+2)(3)", "1+*2",
        ] {
            assert!(!ok(ARITH, "Expr", &lex_arith(s)), "拒否すべき: {s}");
        }
    }

    #[test]
    fn precedence_is_in_the_grammar_layers() {
        // 優先順位を層で表した文法と、層のない曖昧な文法は同じ言語を受理する。
        // 層のある文法では、乗算の項に加算を直接置けない（括弧が必要）。
        let layered = "
            Sum ::= Prod | Sum '+' Prod
            Prod ::= Atom | Prod '*' Atom
            Atom ::= 'n' | '(' Sum ')'
        ";
        assert!(ok(layered, "Sum", &words("n + n * n")));
        assert!(ok(layered, "Sum", &words("( n + n ) * n")));
        assert!(!ok(layered, "Sum", &words("n * + n")));
        assert!(!ok(layered, "Sum", &words("( n + n * n")));
    }

    #[test]
    fn character_level_lexical_rules() {
        let src = r#"
            IDENT   ::= IDSTART IDCHAR*
            IDSTART ::= [a-zA-Z_]
            IDCHAR  ::= IDSTART | [0-9]
            STRING  ::= '"' STRCH* '"'
            STRCH   ::= [^"\\] | '\' ["\\]
            NOQ     ::= [a-z] - 'q'
            HEXPAIR ::= #x41 [#x61-#x63]
            KEYWORD ::= "while" | 'do'
        "#;
        let roots = ["IDENT", "STRING", "NOQ", "HEXPAIR", "KEYWORD"];
        let g = Grammar::parse(src, &roots).unwrap();
        let rec = |start: &str, s: &str| {
            let bnf = g.compile(start).unwrap();
            Earley::new(&bnf).recognize(&chars(s)).is_ok()
        };
        assert!(rec("IDENT", "foo_1"));
        assert!(rec("IDENT", "_"));
        assert!(!rec("IDENT", "1foo"));
        assert!(!rec("IDENT", ""));
        assert!(rec("STRING", r#""a b""#));
        assert!(rec("STRING", r#""a\"b""#));
        assert!(!rec("STRING", r#""a"b""#));
        assert!(!rec("STRING", r#""abc"#));
        assert!(rec("NOQ", "a"));
        assert!(!rec("NOQ", "q"));
        assert!(!rec("NOQ", "ab"));
        assert!(rec("HEXPAIR", "Ab"));
        assert!(!rec("HEXPAIR", "AB"));
        assert!(rec("KEYWORD", "while"));
        assert!(rec("KEYWORD", "do"));
        assert!(!rec("KEYWORD", "whil"));
        // 日本語の 1 文字も 1 トークン。
        let jp = Grammar::parse("JP ::= [ぁ-ん]+", &["JP"]).unwrap();
        let bnf = jp.compile("JP").unwrap();
        assert!(Earley::new(&bnf).recognize(&chars("あいう")).is_ok());
        assert!(Earley::new(&bnf).recognize(&chars("あaう")).is_err());
    }

    #[test]
    fn empty_and_nullable_rules() {
        let src = "
            Start ::= Opt Opt Opt 'x' Tail
            Opt   ::= 'y'?
            Tail  ::= Nul Nul2
            Nul   ::= Nul2 Nul2
            Nul2  ::=
        ";
        assert!(ok(src, "Start", &words("x")));
        assert!(ok(src, "Start", &words("y x")));
        assert!(ok(src, "Start", &words("y y y x")));
        assert!(!ok(src, "Start", &words("y y y y x")));
        assert!(!ok(src, "Start", &words("")));
        assert!(!ok(src, "Start", &words("x y")));
        // 全体が空になれる
        let e = "Start ::= Inner*  Inner ::= 'a'?";
        assert!(ok(e, "Start", &Vec::<&str>::new()));
        assert!(ok(e, "Start", &words("a a a")));
        // 空の代替（末尾の `|`）
        let alt = "Start ::= 'a' Start |";
        assert!(ok(alt, "Start", &words("a a a")));
        assert!(ok(alt, "Start", &Vec::<&str>::new()));
        assert!(!ok(alt, "Start", &words("a b")));
    }

    #[test]
    fn left_and_right_recursion() {
        let left = "List ::= List 'a' | 'a'";
        let right = "List ::= 'a' List | 'a'";
        for n in [1usize, 2, 50, 1000] {
            let toks: Vec<&str> = vec!["a"; n];
            assert!(ok(left, "List", &toks));
            assert!(ok(right, "List", &toks));
        }
        assert!(!ok(left, "List", &Vec::<&str>::new()));
        assert!(!ok(right, "List", &words("a a b")));
        // 中心再帰
        let mid = "Pal ::= 'a' Pal 'a' | 'b' Pal 'b' | 'c' | ";
        assert!(ok(mid, "Pal", &words("a b c b a")));
        assert!(ok(mid, "Pal", &words("a b b a")));
        assert!(!ok(mid, "Pal", &words("a b a b")));
        // 間接の左再帰
        let ind = "Aa ::= Bb 'x' | 'y'  Bb ::= Aa 'z'";
        assert!(ok(ind, "Aa", &words("y z x z x")));
        assert!(!ok(ind, "Aa", &words("y z z x")));
    }

    #[test]
    fn ambiguous_grammars() {
        let amb = "Exp ::= Exp '+' Exp | Exp '*' Exp | 'a'";
        assert!(ok(amb, "Exp", &words("a + a * a + a * a")));
        assert!(!ok(amb, "Exp", &words("a + + a")));
        // 同じ範囲に複数の導出がある（Dangling else）
        let dangling = "
            Stmt ::= 'if' 'c' 'then' Stmt | 'if' 'c' 'then' Stmt 'else' Stmt | 's'
        ";
        assert!(ok(dangling, "Stmt", &words("if c then if c then s else s")));
        assert!(!ok(dangling, "Stmt", &words("if c then else s")));
        // 無限に多義（空に導かれる循環）でも終わる
        let cyc = "Cyc ::= Cyc | Cyc Cyc | 'a' |";
        assert!(ok(cyc, "Cyc", &words("a a a")));
        assert!(ok(cyc, "Cyc", &Vec::<&str>::new()));
    }

    #[test]
    fn not_start_gate() {
        // 'a' の次が 'b' で始まる枝を締め出す。
        let gated = "
            Start ::= 'a' [ ns: 'b' ] Rest
            Rest  ::= 'b' | 'c'
        ";
        let open = "
            Start ::= 'a' Rest
            Rest  ::= 'b' | 'c'
        ";
        assert!(ok(open, "Start", &words("a b")));
        assert!(!ok(gated, "Start", &words("a b")));
        assert!(ok(gated, "Start", &words("a c")));
        // 入力の終わりでは「X でない」が成り立つ。
        assert!(ok("Start ::= 'a' [ ns: 'b' ]", "Start", &words("a")));
        // 選択の片方だけを締め出す
        let alt = "
            Start ::= 'k' ( [ ns: 'b' ] Word | 'b' 'b' )
            Word  ::= 'b' | 'c'
        ";
        assert!(!ok(alt, "Start", &words("k b")));
        assert!(ok(alt, "Start", &words("k c")));
        assert!(ok(alt, "Start", &words("k b b")));
        // 文字クラスの注釈（文字単位）
        let lex = "WORD ::= 'a' [ ns: [0-9] ] [a-z0-9]";
        let g = Grammar::parse(lex, &["WORD"]).unwrap();
        let bnf = g.compile("WORD").unwrap();
        assert!(Earley::new(&bnf).recognize(&chars("ab")).is_ok());
        assert!(Earley::new(&bnf).recognize(&chars("a1")).is_err());
    }

    #[test]
    fn not_followed_gate() {
        // 最長一致: Run は 'a' が続かないところで終わる。
        let longest = "
            Start ::= Run Other
            Run   ::= 'a'+ [ nf: 'a' ]
            Other ::= 'b'*
        ";
        assert!(ok(longest, "Start", &words("a a a b b")));
        assert!(ok(longest, "Start", &words("a a a")));
        // nf が無ければ、Run 'a' の分割で受理される。ある場合は常に 'a' が続くので空の言語。
        let without = "Start ::= Run 'a'  Run ::= 'a'+";
        let with = "Start ::= Run 'a'  Run ::= 'a'+ [ nf: 'a' ]";
        assert!(ok(without, "Start", &words("a a a")));
        assert!(!ok(with, "Start", &words("a a a")));
        assert!(!ok(with, "Start", &words("a a")));
        // 入力の終わりでは成り立つ
        assert!(ok("Start ::= 'a' [ nf: 'b' ]", "Start", &words("a")));
        assert!(!ok(
            "Start ::= 'a' [ nf: 'b' ] Tail  Tail ::= 'b'?",
            "Start",
            &words("a b")
        ));
        // トークン種別（字句規則）を条件にできる
        let kind = "
            Stmt ::= 'x' [ nf: NUMBER ] Tail
            Tail ::= ';' | NUMBER
            NUMBER ::= [0-9]+
        ";
        let num = |t: &str| Tok {
            kind: "NUMBER",
            text: t.into(),
        };
        let sym = |t: &str| Tok {
            kind: "",
            text: t.into(),
        };
        assert!(ok(kind, "Stmt", &[sym("x"), sym(";")]));
        assert!(!ok(kind, "Stmt", &[sym("x"), num("1")]));
    }

    #[test]
    fn annotation_that_makes_a_symbol_nullable() {
        // 空に導かれる記号が注釈だけからなる。入力に依存して空になれるかが決まる。
        let src = "
            Start ::= Guard 'x'
            Guard ::= [ ns: 'y' ]
        ";
        assert!(ok(src, "Start", &words("x")));
        assert!(!ok(src, "Start", &words("y")));
        assert!(!ok(src, "Start", &words("y x")));
        let mid = "
            Start ::= 'p' Guard Other
            Guard ::= [ ns: 'y' ] [ ns: 'z' ]
            Other ::= 'x' | 'y' | 'z'
        ";
        assert!(ok(mid, "Start", &words("p x")));
        assert!(!ok(mid, "Start", &words("p y")));
        assert!(!ok(mid, "Start", &words("p z")));
        // 入れ子でも同じ集合の中で進む
        let nested = "
            Start ::= Outer 'x'
            Outer ::= Guard Guard
            Guard ::= [ ns: 'y' ]
        ";
        assert!(ok(nested, "Start", &words("x")));
        assert!(!ok(nested, "Start", &words("y x")));
    }

    #[test]
    fn grammar_load_errors() {
        // 未定義の記号
        let e = load_errors("Start ::= Missing", &["Start"]);
        assert!(
            e.iter()
                .any(|m| m.contains("未定義") && m.contains("Missing")),
            "{e:?}"
        );
        // 到達しない規則
        let e = load_errors("Start ::= 'a'  Orphan ::= 'b'", &["Start"]);
        assert!(
            e.iter()
                .any(|m| m.contains("到達しません") && m.contains("Orphan")),
            "{e:?}"
        );
        // 構文規則の本体に文字クラス / #x / 差
        let e = load_errors("Start ::= [a-z]", &["Start"]);
        assert!(e.iter().any(|m| m.contains("文字クラス")), "{e:?}");
        let e = load_errors("Start ::= #x41", &["Start"]);
        assert!(e.iter().any(|m| m.contains("文字クラス")), "{e:?}");
        let e = load_errors("Start ::= 'a' - 'b'", &["Start"]);
        assert!(e.iter().any(|m| m.contains("差")), "{e:?}");
        // 字句規則から構文規則
        let e = load_errors("TOKEN ::= Syn  Syn ::= 'a'", &["TOKEN"]);
        assert!(
            e.iter().any(|m| m.contains("字句規則から構文規則")),
            "{e:?}"
        );
        // 二重定義
        let e = load_errors("Start ::= 'a'  Start ::= 'b'", &["Start"]);
        assert!(e.iter().any(|m| m.contains("二重")), "{e:?}");
        // 名前の付け方
        let e = load_errors("start ::= 'a'", &["start"]);
        assert!(e.iter().any(|m| m.contains("UPPER_CASE")), "{e:?}");
        // 入口が無い
        let e = load_errors("Start ::= 'a'", &["Nope"]);
        assert!(e.iter().any(|m| m.contains("入口")), "{e:?}");
        // 差と注釈の項は終端に限る
        let e = load_errors("Start ::= 'a' [ ns: Other ]  Other ::= 'b'", &["Start"]);
        assert!(e.iter().any(|m| m.contains("終端")), "{e:?}");
        let e = load_errors("TOK ::= ( 'a' 'b' ) - 'c'", &["TOK"]);
        assert!(!e.is_empty(), "{e:?}");
        // 書式の誤り
        for bad in [
            "Start ::= 'a",
            "Start ::= ( 'a'",
            "Start ::= /* x",
            "Start ::= 'a' ]",
            "Start := 'a'",
            "Start ::= [a-",
            "Start ::= [z-a]",
            "Start ::= [ ns: 'a'",
            "'a'",
        ] {
            assert!(
                !load_errors(bad, &["Start"]).is_empty(),
                "エラーのはず: {bad}"
            );
        }
        // 正しい文法は通る。コメント、二重引用符、複数行。
        let good = "Start ::= /* c1 */ \"a\" /* c2 */\n  | 'b' Start2\nStart2 ::= 'c'";
        assert!(load_errors(good, &["Start"]).is_empty());
        // 複数のエラーをまとめて返す
        let e = load_errors("Start ::= Aa Bb  Cc ::= 'x'", &["Start"]);
        assert!(e.len() >= 3, "{e:?}");
        // 注釈の `[ns` が文字クラスと取り違えられない
        assert!(load_errors("TOK ::= [ns]", &["TOK"]).is_empty());
        assert!(load_errors("TOK ::= [nsx:]", &["TOK"]).is_empty());
    }

    #[test]
    fn layer_by_name() {
        use super::ebnf::{layer_of, Layer};
        assert_eq!(layer_of("NUMBER"), Some(Layer::Lexical));
        assert_eq!(layer_of("ID_2"), Some(Layer::Lexical));
        assert_eq!(layer_of("Expr"), Some(Layer::Syntax));
        assert_eq!(layer_of("E"), Some(Layer::Lexical));
        assert_eq!(layer_of("expr"), None);
        assert_eq!(layer_of("_X"), None);
    }

    #[test]
    fn syntax_rule_references_lexical_kind_not_its_body() {
        // 構文規則から見た NUMBER は、トークンの種別。本体の文字クラスは字句解析器の仕事。
        let toks = [Tok {
            kind: "NUMBER",
            text: "abc".into(),
        }];
        assert!(ok("Start ::= NUMBER  NUMBER ::= [0-9]+", "Start", &toks));
        let toks = [Tok {
            kind: "",
            text: "123".into(),
        }];
        assert!(!ok("Start ::= NUMBER  NUMBER ::= [0-9]+", "Start", &toks));
    }

    #[test]
    fn deep_group_nesting_in_grammar_text_is_limited() {
        let depth = 200;
        let src = format!("Start ::= {}'a'{}", "(".repeat(depth), ")".repeat(depth));
        assert!(ok(&src, "Start", &words("a")));
        let depth = 100_000;
        let src = format!("Start ::= {}'a'{}", "(".repeat(depth), ")".repeat(depth));
        let e = load_errors(&src, &["Start"]);
        assert!(
            e.iter().any(|m| m.contains("入れ子")),
            "深すぎる入れ子を報告する"
        );
    }

    #[test]
    fn nested_parentheses_ten_thousand_deep() {
        let g = "Group ::= '(' Group ')' |";
        let n = 10_000;
        let mut toks: Vec<&str> = vec!["("; n];
        toks.extend(vec![")"; n]);
        assert!(ok(g, "Group", &toks));
        // 閉じ忘れ・閉じ過ぎ
        toks.pop();
        assert!(run(g, "Group", &toks).is_err());
        let mut toks2: Vec<&str> = vec!["("; n];
        toks2.extend(vec![")"; n + 1]);
        assert_eq!(run(g, "Group", &toks2), Err(RecognizeError { pos: 2 * n }));
        // 入れ子の列
        let g2 = "Seq ::= Seq Group | Group  Group ::= '(' Seq? ')'";
        let mut t: Vec<&str> = vec!["("; n];
        t.extend(vec![")"; n]);
        t.extend(["(", ")"]);
        assert!(ok(g2, "Seq", &t));
    }

    #[test]
    fn error_position_is_reported() {
        let toks = lex_arith("1 + 2 * * 3");
        assert_eq!(run(ARITH, "Expr", &toks), Err(RecognizeError { pos: 4 }));
        let toks = lex_arith("1 + (2");
        assert_eq!(run(ARITH, "Expr", &toks), Err(RecognizeError { pos: 4 }));
    }

    /// 項目数が入力の長さに比例する（壁時計の閾値は assert しない）。
    #[test]
    fn linear_in_input_size_for_left_recursion() {
        let g = Grammar::parse(ARITH, &["Expr"]).unwrap();
        let bnf = g.compile("Expr").unwrap();
        let earley = Earley::new(&bnf);
        let mut per_token = Vec::new();
        for reps in [10_000usize, 20_000, 40_000, 100_000 / 2] {
            // 1+2*3+ ... 形（5 トークン × reps ≒ 2 * reps * 2.5 ...）
            let mut s = String::from("1");
            for _ in 0..reps {
                s.push_str("+2*3");
            }
            let toks = lex_arith(&s);
            let t = Instant::now();
            let stats = earley.recognize(&toks).unwrap();
            let ms = t.elapsed().as_millis();
            eprintln!(
                "arith tokens={} items={} max_set={} time={}ms",
                toks.len(),
                stats.items,
                stats.max_set,
                ms
            );
            assert!(stats.max_set < 100, "集合の大きさが入力に依存しない");
            per_token.push(stats.items as f64 / toks.len() as f64);
        }
        let (lo, hi) = per_token
            .iter()
            .fold((f64::MAX, 0.0f64), |(l, h), &x| (l.min(x), h.max(x)));
        assert!(
            hi / lo < 1.05,
            "トークンあたりの項目数が一定: {per_token:?}"
        );
    }

    #[test]
    fn hundred_thousand_tokens() {
        let g = Grammar::parse(ARITH, &["Expr"]).unwrap();
        let bnf = g.compile("Expr").unwrap();
        let mut s = String::from("(1)");
        // 25_000 × 4 + 3 = 100_003 トークン
        for _ in 0..25_000 {
            s.push_str("+2*3");
        }
        let toks = lex_arith(&s);
        assert!(toks.len() >= 100_000);
        let t = Instant::now();
        let stats = Earley::new(&bnf).recognize(&toks).unwrap();
        eprintln!(
            "100k arith tokens={} items={} time={}ms",
            toks.len(),
            stats.items,
            t.elapsed().as_millis()
        );
        // 左再帰の連 100k
        let g = Grammar::parse("List ::= List 'a' | 'a'", &["List"]).unwrap();
        let bnf = g.compile("List").unwrap();
        let toks: Vec<&str> = vec!["a"; 100_000];
        let t = Instant::now();
        let stats = Earley::new(&bnf).recognize(&toks).unwrap();
        eprintln!(
            "100k left-recursive list items={} time={}ms",
            stats.items,
            t.elapsed().as_millis()
        );
        assert!(stats.items < 10 * toks.len());
        // `*` の展開（左再帰）でも同じ
        let g = Grammar::parse("List ::= 'a'+", &["List"]).unwrap();
        let bnf = g.compile("List").unwrap();
        let t = Instant::now();
        let stats = Earley::new(&bnf).recognize(&toks).unwrap();
        eprintln!(
            "100k plus-expanded items={} time={}ms",
            stats.items,
            t.elapsed().as_millis()
        );
        assert!(stats.items < 10 * toks.len());
        // 入力の 1 か所が誤りなら位置を報告する
        let mut bad = toks.clone();
        bad[77_777] = "b";
        assert_eq!(
            Earley::new(&bnf).recognize(&bad).map(|_| ()),
            Err(RecognizeError { pos: 77_777 })
        );
    }

    #[test]
    fn star_and_plus_expand_to_left_recursion() {
        let g = Grammar::parse("List ::= 'a'* 'b'+ ('c' | 'd')?", &["List"]).unwrap();
        let bnf = g.compile("List").unwrap();
        // 繰り返しの補助規則は、自分自身を右辺の先頭に持つ（左再帰）。
        let left_rec = bnf
            .rules
            .iter()
            .filter(|r| r.rhs.first() == Some(&super::ebnf::Sym::Nt(r.lhs)))
            .count();
        assert_eq!(left_rec, 2);
        let right_rec = bnf
            .rules
            .iter()
            .filter(|r| r.rhs.len() > 1 && r.rhs.last() == Some(&super::ebnf::Sym::Nt(r.lhs)))
            .count();
        assert_eq!(right_rec, 0);
        let e = Earley::new(&bnf);
        assert!(e.recognize(&words("a a b b c")).is_ok());
        assert!(e.recognize(&words("b d")).is_ok());
        assert!(e.recognize(&words("b")).is_ok());
        assert!(e.recognize(&words("a a")).is_err());
        assert!(e.recognize(&words("b c d")).is_err());
    }
}
