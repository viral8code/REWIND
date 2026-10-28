import sys


def load(p):
    s = open(p, newline="", encoding="utf-8").read()
    return s.replace("\r\n", "\n"), "\r\n" in s


def save(p, s, crlf):
    if crlf:
        s = s.replace("\n", "\r\n")
    open(p, "w", newline="", encoding="utf-8").write(s)


p = "src/v2.rs"
s, crlf = load(p)


def rep(a, b):
    global s
    assert s.count(a) == 1, (a, s.count(a))
    s = s.replace(a, b)


rep(
    """    let value = if negative {
        -(magnitude as i128)
    } else {
        magnitude as i128
    };
    i64::try_from(value).map_err(|_| diagnostic(at, "integer literal overflow"))""",
    """    // `0xFE..FE` with the top bit set is the two's complement of a negative number, as in C and Rust.
    let based = at.text.len() > 1
        && at.text.starts_with('0')
        && matches!(at.text.as_bytes()[1], b'x' | b'X' | b'b' | b'B' | b'o' | b'O');
    if based && !negative && magnitude > i64::MAX as u64 {
        return Ok(magnitude as i64);
    }
    let value = if negative {
        -(magnitude as i128)
    } else {
        magnitude as i128
    };
    i64::try_from(value).map_err(|_| diagnostic(at, "integer literal overflow"))""",
)
rep(
    """                    text.push(it.next().unwrap());
                    col += 1;
                }
            }
            if text.chars().count() == 1 && !"=+-*/%(),;{}.!<>?:[]|&^".contains(c) {""",
    """                    text.push(it.next().unwrap());
                    col += 1;
                    // `>>>` (logical shift); `>>>=` stays `>>` + `>=`.
                    if text == ">>" {
                        let mut ahead = it.clone();
                        if ahead.next() == Some('>') && ahead.peek() != Some(&'=') {
                            text.push(it.next().unwrap());
                            col += 1;
                        }
                    }
                }
            }
            if text.chars().count() == 1 && !"=+-*/%(),;{}.!<>?:[]|&^~".contains(c) {""",
)
rep(
    """        if s == ">" && (self.is(">=") || self.is(">>")) {
            let (original, rest) = if self.is(">=") {
                (">=", "=")
            } else {
                (">>", ">")
            };""",
    """        if s == ">" && (self.is(">=") || self.is(">>") || self.is(">>>")) {
            let (original, rest) = if self.is(">=") {
                (">=", "=")
            } else if self.is(">>") {
                (">>", ">")
            } else {
                (">>>", ">>")
            };""",
)
rep('"<<" | ">>" => 8,', '"<<" | ">>" | ">>>" => 8,')
rep(
    '"-" | "!" | "await" | "spawn" | "move" => Expr {',
    '"-" | "!" | "~" | "await" | "spawn" | "move" => Expr {',
)
rep(
    """                if op == "!" && t != "Bool" || op == "-" && !matches!(t.as_str(), "Int" | "Float") {""",
    """                if op == "!" && t != "Bool"
                    || op == "-" && !matches!(t.as_str(), "Int" | "Float")
                    || op == "~" && t != "Int"
                {""",
)
rep(
    '''                    "&" | "|" | "^" | "<<" | ">>" => {
                        if x != "Int" || y != "Int" {''',
    '''                    "&" | "|" | "^" | "<<" | ">>" | ">>>" => {
                        if x != "Int" || y != "Int" {''',
)
rep(
    """                    ("!", Value::Bool(n)) => Ok(Value::Bool(!n)),
                    ("move" | "borrow" | "borrowMut", v) => Ok(v),""",
    """                    ("!", Value::Bool(n)) => Ok(Value::Bool(!n)),
                    ("~", Value::Int(n)) => Ok(Value::Int(!n)),
                    ("move" | "borrow" | "borrowMut", v) => Ok(v),""",
)
rep(
    """        ("<<" | ">>", Value::Int(a), Value::Int(b)) => {""",
    """        ("<<" | ">>" | ">>>", Value::Int(a), Value::Int(b)) => {""",
)
rep(
    """            // `<<` drops the bits that leave the 64-bit value; `>>` keeps the sign.
            Ok(Value::Int(if op == "<<" { a << b } else { a >> b }))""",
    """            // `<<` drops the bits that leave the 64-bit value; `>>` keeps the sign; `>>>` shifts in zeros.
            Ok(Value::Int(match op {
                "<<" => a << b,
                ">>" => a >> b,
                _ => ((a as u64) >> b) as i64,
            }))""",
)
rep(
    """                ("abs", []) => "Int".to_string(),
                ("min" | "max" | "pow", [x]) if compatible("Int", x) => "Int".into(),""",
    """                ("abs" | "countOnes" | "trailingZeros" | "leadingZeros" | "lowestBit", []) => {
                    "Int".to_string()
                }
                ("min" | "max" | "pow" | "wrappingAdd" | "wrappingSub" | "wrappingMul", [x])
                    if compatible("Int", x) =>
                {
                    "Int".into()
                }""",
)
rep(
    """            (Value::Int(n), "min", [Value::Int(m)]) =>""",
    """            (Value::Int(n), "countOnes", []) => {
                return Ok(Some(Value::Int(i64::from(n.count_ones()))))
            }
            (Value::Int(n), "trailingZeros", []) => {
                return Ok(Some(Value::Int(i64::from(n.trailing_zeros()))))
            }
            (Value::Int(n), "leadingZeros", []) => {
                return Ok(Some(Value::Int(i64::from(n.leading_zeros()))))
            }
            // The lowest set bit (0 for 0); `v & (0 - v)` would overflow for the sign bit.
            (Value::Int(n), "lowestBit", []) => {
                return Ok(Some(Value::Int(n & n.wrapping_neg())))
            }
            (Value::Int(n), "wrappingAdd", [Value::Int(m)]) => {
                return Ok(Some(Value::Int(n.wrapping_add(*m))))
            }
            (Value::Int(n), "wrappingSub", [Value::Int(m)]) => {
                return Ok(Some(Value::Int(n.wrapping_sub(*m))))
            }
            (Value::Int(n), "wrappingMul", [Value::Int(m)]) => {
                return Ok(Some(Value::Int(n.wrapping_mul(*m))))
            }
            (Value::Int(n), "min", [Value::Int(m)]) =>""",
)
rep(
    '''"slice", "extend", "toUpper", "toLower",
];''',
    '''"slice", "extend", "toUpper", "toLower", "countOnes", "trailingZeros", "leadingZeros", "lowestBit",
    "wrappingAdd", "wrappingSub", "wrappingMul",
];''',
)
save(p, s, crlf)

p = "src/v2/editor.rs"
s, crlf = load(p)
rep(
    'const OPERATOR_CHARACTERS: &str = "=+-*/%.!<>?:|&^";',
    'const OPERATOR_CHARACTERS: &str = "=+-*/%.!<>?:|&^~";',
)
rep(
    """            let width = if TWO_CHARACTER_OPERATORS.contains(&two.as_str()) && !shift_assign {
                2
            } else {
                1
            };""",
    """            let triple = two == ">>" && at(i + 2) == Some('>') && at(i + 3) != Some('=');
            let width = if triple {
                3
            } else if TWO_CHARACTER_OPERATORS.contains(&two.as_str()) && !shift_assign {
                2
            } else {
                1
            };""",
)
save(p, s, crlf)

p = "src/v2/grammar_check/syntaxcheck.rs"
s, crlf = load(p)
rep(
    """        } else if t == ">>" {""",
    """        } else if t == ">>>" {
            for kind in ["USR_FIRST", "USR_SECOND", "USR_THIRD"] {
                out.push(GTok {
                    kind,
                    text: ">".into(),
                });
            }
        } else if t == ">>" {""",
)
rep(
    """                            ">" | ">>" => {
                                d -= if t[j] == ">>" { 2 } else { 1 };""",
    """                            ">" | ">>" | ">>>" => {
                                d -= t[j].len() as i32;""",
)
save(p, s, crlf)

p = "src/v2/grammar_check/lexcheck.rs"
s, crlf = load(p)
rep(
    '''(">> << >>= <<= >>>", Some(vec![">>", "<<", ">", ">=", "<", "<=", ">>", ">"])),''',
    '''(">> << >>= <<= >>>", Some(vec![">>", "<<", ">", ">=", "<", "<=", ">>>"])),
            (">>>=", Some(vec![">>", ">="])),
            ("~a", Some(vec!["~", "a"])),''',
)
save(p, s, crlf)

p = "docs/grammar/rewind.ebnf"
s, crlf = load(p)
rep(
    "SYMBOL ::= SYMBOL2 | SYMBOL1",
    "SYMBOL ::= SYMBOL3 | SYMBOL2 | SYMBOL1\n\nSYMBOL3 ::= '>>>' [ nf: [=] ]",
)
rep("[=+\\-*%(),;{}.!<>?:\\[\\]|&^]", "[=+\\-*%(),;{}.!<>?:\\[\\]|&^~]")
rep(
    "ShiftOp ::= '<<' | SHR_FIRST SHR_SECOND",
    "ShiftOp ::= '<<' | SHR_FIRST SHR_SECOND | USR_FIRST USR_SECOND USR_THIRD",
)
rep(
    "UnaryExpr ::= '-' UnaryExpr\n            | '!' UnaryExpr\n",
    "UnaryExpr ::= '-' UnaryExpr\n            | '!' UnaryExpr\n            | '~' UnaryExpr\n",
)
rep(
    "CUnaryExpr ::= '-' CUnaryExpr\n             | '!' CUnaryExpr\n",
    "CUnaryExpr ::= '-' CUnaryExpr\n             | '!' CUnaryExpr\n             | '~' CUnaryExpr\n",
)
rep(
    "SHR_SECOND ::= '>'\n",
    "SHR_SECOND ::= '>'\n\nUSR_FIRST ::= '>'\n\nUSR_SECOND ::= '>'\n\nUSR_THIRD ::= '>'\n",
)
save(p, s, crlf)
print("ok")
