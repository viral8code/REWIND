## 第22章 テスト・診断・記録と再生

### テストを書く

`test fn` で書いた関数は、通常の実行では呼ばれません。`rewind test` が、それらを一つずつ実行します。中では `assert` と `assert_eq` で結果を確かめます。

```rewind
fn double(x: Int) -> Int = x * 2;

test fn doubles_small_numbers() {
    assert_eq(double(4), 8);
}

test fn doubles_zero() {
    assert_eq(double(0), 0);
}

fn main()->Int {
    Out.println(double(21));
    return 0;
}
```

```text
42
```

`rewind test` は、プロジェクト（`rewind.toml` があるディレクトリ）に対して実行します。最初に一度、`rewind update` でロックファイルを作っておく必要があります。

```sh
rewind update
rewind test --root .
```

```text
ok doubles_small_numbers
ok doubles_zero
```

成功したテストは、`ok 名前` と表示されます。失敗すると、そこで止まり、失敗した位置と、左右の値が表示されます。

```text
rewind: invalid operation: test doubles_zero: invalid operation: main.rw:8:5: assert_eq failed: left=0 right=1
```

`--filter 名前` を付けると、名前に一致するテストだけを実行します。テストの中では `publish` を使えません。テストごとに独立した仮想ファイルが使われるので、あるテストが書いたファイルを、別のテストから読むことはできません。

`assert_eq` が左右を比べる規則は、`==` と同じです（第3章）。`List` どうしを比べたいときは、`freeze` してから比べてください。

### エラーの診断を読む

実行時のエラーは、エラーのコード、位置、呼び出しの経路、対処のヒントの順に表示されます。

```rewind-error DivisionByZero
fn divide(a: Int, b: Int) -> Int effects {} {
    return a / b;
}

fn main()->Int {
    Out.println(divide(10, 0));
    return 0;
}
```

```text
error[DivisionByZero]: division by zero
  --> bad.rw:2:12
  at divide (bad.rw:2:12)
  at main (bad.rw:6:17)
  at <entry> (bad.rw:5:1)
  help: Check the divisor before division or remainder.
```

- `-->` の行は、エラーが起きた `ファイル:行:列` です。行と列は 1 から数えます。
- `at` の行は、内側の呼び出しから外側へ向かう経路です。最大 32 件までで、それ以上は省略されます。変数や引数の値は表示されません。
- `help` は、対処のヒントです。

主なエラーのコードを挙げます。

| コード | 主な原因 |
|---|---|
| `DivisionByZero` | 整数を 0 で割った |
| `IntegerOverflow` | `Int` の範囲を超えた |
| `IndexOutOfBounds` | 範囲外の添字 |
| `AssertionFailed`、`Panic` | `assert`、`assert_eq`、`panic` |
| `UseAfterMove` | `move` した後の値を使った |
| `MissingEffect` | 効果の宣言または許可が足りない |
| `InvalidContinuation` | `revert` が戻れない保存点を指した |
| `ExternalBoundary` | 外部領域で禁止されている操作をした |
| `ExecutionBudgetExceeded` ほか | 実行予算の超過（第23章） |

プログラムが終了したときの **終了コード** も、原因を知る手がかりです。

| 終了コード | 意味 |
|---|---|
| 0 | 正常終了 |
| 1〜63 | `main` が返した値 |
| 64 | コマンドの使い方の誤りなど |
| 65 | 型・所有権の検査エラー、アサーションの失敗、実行時の失敗 |
| 70 | `panic` や効果の不足など、原因を伴う失敗 |
| 71 | 予算の超過 |
| 72 | 外部の状態の衝突 |
| 73 | 公開が途中まで適用された |
| 74 | タスクの取り消し |

ツールから結果を読み取りたいときは、任意のコマンドに `--diagnostic-format json` を付けます。診断が JSON として標準エラーに出力され、標準出力にはプログラムの出力だけが残ります。`diagnostic` には `code`、`message`、`line`、`column`、`frames`、`hints` などが入っています。

### 実行を記録して、再生する

`--record` を付けて実行すると、外から得た値（標準入力、時刻、乱数、ファイルの読み取り、外部領域の結果）が記録されます。`rewind replay` は、その記録を使って同じ実行を再現し、結果が記録と一致するかを確かめます。再生では、実際の入力や時計は使いません。

```rewind stdin=REWIND\n
fn main()->Int effects {input, output} {
    match In.readLine() {
        Some(name) => { Out.println("こんにちは、" + name + "さん"); },
        None => { Out.println("名前がありません"); }
    }
    return 0;
}
```

```text
こんにちは、REWINDさん
```

```sh
printf 'REWIND\n' | rewind run main.rw --record trace.json
rewind replay trace.json --root .
```

一度目は標準入力から読み、二度目は記録から入力を取り出します。二度目には標準入力を渡していませんが、同じ出力になります。

この仕組みは、「再現しにくい不具合を、一度捕まえたら何度でも再現する」ために役立ちます。本番で失敗した実行を記録しておけば、手元で同じ失敗を再生して、原因を調べられます。

記録には二つの方式があります。

| 方式 | 内容 |
|---|---|
| `--record-mode debug`（既定） | 命令ごとの状態の索引を持つ。デバッガで巻き戻しながらステップ実行できる。大きく、長い実行には向かない |
| `--record-mode compact` | 命令数、実行順序のダイジェスト、観測、最終状態だけ。長い実行向き |

秘密の値（`Secret`、`--secret-env`、`In.readSecretLine()`）は記録に残りません。記録の形式は、コンパイラの版をまたいで互換ではありません。版を上げたら、記録し直してください。

### 仕事量を調べる

`rewind profile` は、関数ごと、行ごとの命令数とネイティブ処理量、実行されなかった行（カバレッジ）を表示します。測るのは時間ではなく、第23章の予算で数える **論理的な仕事量** です。既定の出力は JSON で、`--profile-format text` で読みやすい形になります。

```rewind
fn fib(n: Int) -> Int effects {} {
    if n < 2 { return n; }
    return fib(n - 1) + fib(n - 2);
}

fn main()->Int {
    Out.println(fib(15));
    return 0;
}
```

```text
610
```

```sh
rewind profile main.rw --profile-format text
```

```text
610
task 0 main: 6 instructions exclusive, 22691 inclusive, 10 native work
task 0 fib: 22685 instructions exclusive, 22685 inclusive, 0 native work
task 0 <entry>: 11 instructions exclusive, 11 inclusive, 11 native work
coverage main.rw: 5/5 lines (uncovered: [])
```

`exclusive` は、その関数が直接実行した命令の数、`inclusive` は、呼び出した先も含めた数です。再帰した関数の `inclusive` は、他の関数の値と足し合わせてはいけません。この例では、`fib` が命令のほとんどを使っていると、一目で分かります。

### エディタとデバッガ

| 道具 | 使い方 |
|---|---|
| LSP | `rewind lsp --root DIR` で起動する。診断、補完、ホバー、定義・参照、名前の変更、整形に対応 |
| デバッガ（DAP） | `--record` で記録した実行を、逆実行を含めてステップ実行できる。[debugger.md](../debugger.md) |
| 整形 | `rewind fmt main.rw`。`--check` を付けると差分の有無だけ調べる |
| 互換性の確認 | `rewind compat` は、コンパイラ、言語、成果物、記録などの版の関係を表示する |
