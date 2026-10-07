# REWIND 1.9.33 言語リファレンス

対象はcompiler/language 1.9.33です。これは現在実装されている構文と動作の説明です。過去の草案は採用されなかった案も含むため、この文書と[1.0の保証範囲](REWIND_v1.0.md)、[1.1の変更点](REWIND_v1.1.md)、[1.2の変更点](REWIND_v1.2.md)、[1.3の変更点](REWIND_v1.3.md)、[1.4の変更点](REWIND_v1.4.md)を基準にしてください。

- [実行とツール](#実行とツール)
- [字句と基本型](#字句と基本型)
- [変数と演算](#変数と演算)
- [制御構文](#制御構文)
- [関数とエントリーポイント](#関数とエントリーポイント)
- [データ型](#データ型)
- [所有権と借用](#所有権と借用)
- [OptionとResult](#optionとresult)
- [クロージャーとIterator](#クロージャーとiterator)
- [traitとgeneric](#traitとgeneric)
- [moduleと公開範囲](#moduleと公開範囲)
- [効果と外部へのアクセス](#効果と外部へのアクセス)
- [Checkpointとpublish](#checkpointとpublish)
- [cleanupとタスク](#cleanupとタスク)
- [文字列と標準ライブラリ](#文字列と標準ライブラリ)
- [エラーの調べ方](#エラーの調べ方)
- [実行予算と制限](#実行予算と制限)

以下の`rewind`コードブロックは、それぞれ独立したプログラムです。`main.rw`に保存して実行できます。文書の例も回帰テストで実行します。

## 実行とツール

```sh
rewind run main.rw
rewind compile main.rw
rewind run main.rwc
```

compileは既定で同じ場所に`main.rwc`を生成します。成果物はソースファイルなしでも実行できます。`rewindc main.rw`、`rewind main.rwc`も利用できます。単一ファイルの実行にmanifestや事前compileは不要です。SDKのbinをPATHに追加します。Windowsでは同じコマンド名で`.exe`が実行されます。

| 用途 | コマンド |
|---|---|
| バージョン、ヘルプ | `rewind --version`、`rewind --help` |
| 単一ファイルの整形 | `rewind fmt main.rw` |
| 整形の検査のみ | `rewind fmt main.rw --check` |
| エディタ接続 | `rewind lsp --root DIR`（stdio LSP） |
| projectの型・効果検査 | `rewind check --root DIR` |
| projectのテスト | `rewind test --root DIR` |
| 観測と実行の記録 | `rewind run main.rw --record trace.json` |
| 記録の再生 | `rewind replay trace.json --root DIR` |
| JSON診断 | コマンドに`--diagnostic-format json`を付ける |

`rewind run --help`など主要コマンドの`--help` / `-h`でも使用方法を確認できます。`--`以降の`--help`はapplication引数です。

formatterは既存の改行を保ちながらインデントを整えます。任意のコードを一行ずつ分割するformatterではありません。LSPは診断、補完、hover、定義・参照、rename、signature help、既存のquick fix、整形に対応します。エディタ側にはstdio LSPクライアントの設定が必要です。

## 字句と基本型

文末は`;`、ブロックは`{ ... }`です。行コメントは`//`、入れ子にできるブロックコメントは`/* ... */`を使います。文字列は二重引用符で囲み、`\n`、`\r`、`\t`、`\"`、`\\`を使えます。`\0`と`\u{HEX}`も使えます。Unicode escapeは1〜6桁のhexで有効なUnicode scalarを指定します（例：`"\u{754c}"`は`"界"`）。識別子や型名は大文字・小文字を区別します。

| 型 | 意味 |
|---|---|
| `Int` | 符号付き64bit整数。算術の範囲外は実行時エラー |
| `Float` | IEEE 754 binary64。NaN、無限大も表現できる |
| `Bool` | `true`、`false` |
| `String` | UTF-8文字列 |
| `Bytes` | byte列。`Bytes("text")`でUTF-8 byte列を作れる |
| `Unit` | 値`()`。戻り値が不要な関数に使う |
| `Never` | `panic`など通常は戻らない式の型 |
| `List<T>`、`Map<K,V>` | 型付きコレクション |
| `Option<T>`、`Result<T,E>` | 値の有無、成功・失敗 |
| `(A,B)` | tuple。`._0`、`._1`で読む |
| `Frozen<T>` | 共有可能な不変snapshot |
| `Secret<T>` | 診断・記録で値を伏せるための型 |

数値は`1_000`のように数字の間を`_`で区切れます。整数には`0xff`、`0b1010`、`0o755`も使えます。Floatの例は`1_2.5_0e+0_1`です。Intの範囲は表記にかかわらずsigned 64-bitです。

暗黙に任意の型へ変換する仕組みはありません。整数と浮動小数点の境界では`toFloatChecked`、`toIntChecked`等の明示変換を使います。

```rewind
let count:Int = 3;
let ratio:Float = 1.5;
let ready:Bool = true;
let pair:(Int,String) = (count,"three");
assert_eq(pair._0,3);
assert_eq("界".byteLen(),3);
assert_eq("界".charLen(),1);
Out.println("Hello, REWIND!");
publish;
```

```rewind
/* 表記の例。 /* コメントは入れ子にできる */ */
let limit = 1_000;
let mask = 0xff;
assert_eq(limit,1000);
assert_eq(mask,255);
assert_eq(0b1010,10);
assert_eq(0o755,493);
assert_eq("\u{754c}\u{1f600}","界😀");
```

## 変数と演算

`let`はbindingの再代入を禁止し、`var`は許可します。`let`で保持するList等も、所有権・借用検査を満たす場合は内容を更新できます。型が推論できる場合は型注釈を省略します。

```rewind
let base = 10;
var score = base;
score += 5;
let values = List<Int>();
values.add(score);
assert_eq(values.get(0),15);
```

演算子の優先順位は、弱い順に次の通りです。括弧で明示できます。

| 順位 | 演算子 |
|---|---|
| 1 | `||` |
| 2 | `&&` |
| 3 | `==`、`!=` |
| 4 | `<`、`<=`、`>`、`>=` |
| 5 | `+`、`-` |
| 6 | `*`、`/`、`%` |

`&&`、`||`は短絡評価します。整数の除算はゼロ方向へ切り捨て、余りは被除数の符号に従います。整数のゼロ除算は`DivisionByZero`、範囲外は`IntegerOverflow`です。Floatの算術はIEEE 754に従うため、整数と同じエラー規則にはなりません。bit操作は`std.bits`を使います。

## 制御構文

`if` / `else if` / `else`、`while`、半開区間の`for`、`break`、`continue`、`return`があります。条件はBoolです。

```rewind
var sum = 0;
for i in 0..4 {
    if i == 2 { continue; }
    sum += i;
}
while sum < 10 { sum += 1; }
assert_eq(sum,10);
```

`match`は値のvariantや構造に応じて分岐します。enumには原則として網羅的な分岐が必要です。`_`は値を捨てるpatternです。guardも使えますが、guardからpublishやCheckpoint操作を行うことはできません。

v1.9 以降は入れ子の Option / Result、generic enum、tuple、record の組合せも網羅性検査します。guard は網羅性の根拠になりません。先行 arm に含まれる arm、空・逆順の Int 区間を拒否します。解析上限と版の互換性は [v1.9](REWIND_v1.9.md) を参照してください。

## 関数とエントリーポイント

関数は`fn name(params)->ReturnType effects { ... } { ... }`です。再帰も利用できます。

```rewind
fn factorial(n:Int)->Int effects {} {
    if n <= 1 { return 1; }
    return n * factorial(n-1);
}
assert_eq(factorial(5),120);
```

トップレベルに処理を並べる形式に加え、`fn main()->Int`をアプリケーションの入口にできます。mainの0は正常終了、非0は終了値として扱います。mainを使う場合もpending出力にはpublishが必要です。

```rewind
fn main()->Int effects {output} {
    Out.println("started");
    publish;
    return 0;
}
```

## データ型

`struct`は可変のデータ、`record`は共有可能な不変データに使います。recordのfieldはShareを満たす型に限定されます。`enum`は複数のvariantを定義します。tuple、型alias、generic型も使えます。

```rewind
struct Counter { value:Int }
record Point { x:Int,y:Int }
enum Status { Ready,Failed(String) }
type Score = Int;
let counter = Counter(1);
counter.value = 2;
let point = Point(3,4);
assert_eq(point.x+point.y,7);
let status = Status::Failed("missing");
match status {
    Status::Ready => {},
    Status::Failed(message) => { assert_eq(message,"missing"); }
}
```

struct/record/enumは位置引数のconstructorを持ちます。named constructor・field patternも利用できます。fieldを`private`にした型は、そのmodule外から直接construct・read・write・分解できません。公開factoryやaccessorを用意します。

## 所有権と借用

可変ownerは自由に複製できません。所有権を渡す場所では`move`、読むだけなら`&`、更新するなら`&mut`を使います。IntやString、適切なrecord等のShare値は共有できます。bindingの可変性と値の所有権は別です。

```rewind
fn total(values:&List<Int>)->Int effects {} {
    var result=0;
    for i in 0..values.len() { result+=values.get(i); }
    return result;
}
fn append(values:&mut List<Int>,n:Int)->Unit effects {} {
    values.add(n);
}
let values=List<Int>();
append(&mut values,7);
assert_eq(total(&values),7);
let snapshot=freeze(values);
let copy=thaw(snapshot);
copy.add(8);
assert_eq(copy.len(),2);
```

借用中に元のownerを競合する形で変更・moveできません。借用引数を返り値等へ逃がすことにも制限があります。`freeze`は不変snapshot、`thaw`はそこから独立した可変値を作ります。リソース・関数値等は任意にfreezeできません。

## OptionとResult

`Option<T>`は`Some(value)`または`None`、`Result<T,E>`は`Ok(value)`または`Err(error)`です。通常の失敗はResultとして返し、matchで処理します。`?`はResultを返す関数内で同じ失敗型のErrを早期returnします。

```rewind
import std.number as number;
import std.integer as integer;
fn increment(input:String)->Result<Int,String> effects {} {
    let n=number.decimal(input);
    match n {
        Ok(value) => { return integer.add(value,1); },
        Err(_) => { return Err("InvalidNumber"); }
    }
}
fn twice(input:Int)->Result<Int,String> effects {} {
    let value=integer.add(input,input)?;
    return Ok(value);
}
assert_eq(increment("41"),Ok(42));
assert_eq(twice(21),Ok(42));
let item:Option<Int>=Some(7);
match item { Some(n)=>{assert_eq(n,7);},None=>{} }
```

panic・予算超過・通常のList範囲外アクセス等は、任意の関数のResultへ自動変換されません。失敗を回復したい境界では、検査するかResultを返すAPIを選びます。`tryStable`等のfallible callback APIも用意しています。

## クロージャーとIterator

クロージャーは`|n:Int|->Int{ ... }`、引数なしなら`||->Int{ ... }`です。関数型は`fn(Int)->Int effects {}`のように表し、必要に応じて`captures {Send,Share}`の契約を持ちます。明示captureには`capture value`、`capture move`、`capture borrow`があります。借用captureは借用元より長く生存したりtaskへ逃げたりできません。

```rewind
let values=List<Int>();
values.add(1);values.add(2);values.add(3);
let doubled=values.iter()
    .map(|n:Int|->Int{return n*2;})
    .filter(|n:Int|->Bool{return n>2;})
    .collect();
assert_eq(doubled.len(),2);
assert_eq(doubled.get(0),4);
```

Iteratorにはnext/map/filter/fold/take/collect/enumerate/zip等があります。cursorはCheckpointの対象です。callbackの型・効果・借用契約も検査されます。

## traitとgeneric

traitは型が提供する操作の契約です。implで実装します。`T:Ord+Share`のような複数boundや`where`、関連型・default methodを利用できます。genericは必要な型に特殊化され、無制限の型展開には予算があります。

```rewind
trait Read { fn read(self:&Self)->Int effects {}; }
struct Box { value:Int }
impl Read for Box {
    fn read(self:&Box)->Int effects {} { return self.value; }
}
fn read<T:Read>(value:&T)->Int effects {} { return value.read(); }
fn compare<T>(a:T,b:T)->Int effects {} where T:Ord+Share {
    return a.cmp(b);
}
let box=Box(7);
assert_eq(read(&box),7);
assert_eq(compare(1,2),-1);
```

generic factoryの戻り値から全type parameterを確定できる場合、型注釈やreturnの期待型から推論できます。確定できない場合は`List<Int>()`等の明示type argumentを使います。

## moduleと公開範囲

`import std.number as number;`のようにaliasを指定し、`number.decimal(...)`で呼びます。相対moduleもimportできます。選択importとaliasも利用できます。宣言は原則module内で、`pub fn`、`pub struct`等を付けたものを外部へ公開します。

単一ファイルでは標準ライブラリはcompilerに内蔵されています。相対importはsource directoryを基準に解決します。projectではmanifestのsource root、import設定、lockと署名の検証を使います。module aliasはmoduleごとの名前です。

SDKの標準API文書は`share/rewind/doc/std/`にあります。各moduleのMarkdownと`.api.json`には公開signature、generic bound、効果・所有権契約が含まれます。

## 効果と外部へのアクセス

関数の`effects {output}`等は外部操作の契約です。呼び出し経路を含めて検査されます。関数の宣言と、実行者が与える許可の両方が必要です。

| 主な効果 | 操作 |
|---|---|
| `input`、`output` | 標準入力、stdout/stderr |
| `fileRead`、`fileWrite` | 仮想file/directory操作 |
| `args`、`env` | 引数、許可された環境変数 |
| `clock`、`random` | 時刻観測、乱数 |
| `tasks` | task/channel/scheduling |
| `locale` | locale関連操作 |

単一ファイルの既定許可はinput/output/args/locale/random/tasksです。file・env・clock・gui・external・network を使う場合はCLIで明示許可します。例えば次のプログラムには`rewind run main.rw --allow-effects fileRead,fileWrite`を使います。

```rewind
File.writeText("state.txt","saved");
publish;
assert_eq(File.readText("state.txt"),Ok("saved"));
```

Fileのパスは実行rootからの相対パスです。OSに依存しない`/`区切りを使います。絶対パス、`..`、drive指定やroot外へのsymlinkは許可しません。WindowsでCLIへ渡すsource/SDK/rootのパスには通常のWindowsパスを使えます。source内のFileパスと混同しないでください。

`Args.all()`等で渡した引数を読み、アプリ引数はCLIの`--`以降に置きます。Envは効果に加えて名前の許可が必要です。Secretを使っても暗号化や外部への送信防止を自動保証するわけではありません。

## Checkpointとpublish

1.3.0 では、ユーザーコードの実行前に `commit begin;` 相当の初期 Checkpoint が自動保存されます。`begin` は予約名であり、変数・関数・branch の名前や `commit` / `drop` の対象にできません。

`revert begin;` は変数・heap・未公開 I/O・タスク状態・通常の Checkpoint を初期化し、続く文へ進みます。消えた変数は参照できず、同じ名前を宣言し直せます。main task 内で利用し、active branch の外で実行します。関数や block の継続枠だけは保持するので、その後の return / block 終了は可能です。`resume begin;` は実行位置も先頭へ戻します。処理済み予算、Host 観測の記録、publish 済みの外部結果は保持されます。初期 Checkpoint を保持するメモリも履歴予算の対象です。

```rewind
var score=99;
commit test;
Out.println(score);publish;
Out.println("discarded");
revert begin;
var score=10;
commit test;
Out.println(score);publish;
```

この例の出力は `99`、`10` です。未公開の `discarded` は消えます。GUI の View もリセット対象ですが、公開済み画面は再描画または close を publish するまで残ります。

| 構文 | 意味 |
|---|---|
| `commit name;` | 計算状態・未公開I/OのCheckpointを保存 |
| `revert name;` | Checkpointへ作業状態を戻す |
| `resume name;` | Checkpointの制御位置も復元する |
| `drop name;` | 不要なCheckpointを解放 |
| `branch name { ... }` | 隔離した試行を実行し、終了時に親状態へ戻す |
| `publish;` | pending I/Oを外部へ確定 |

```rewind
var score=10;
commit test1;
score=99;
Out.println(score);
commit test2;
publish;
revert test1;
Out.println(score);
publish;
drop test2;
drop test1;
```

出力は`99`、`10`です。既にpublishした出力はrevertで消えず、再送もしません。最初のpublishがない場合、99のpending出力はrevertで破棄されます。commitは外部への確定操作ではありません。

`revert`は実行位置を維持します。同じ関数呼び出しの中では、`if`や入れ子のブロックから外側で作成したCheckpointへ戻せます。変数と未公開I/Oを復元し、Checkpoint後に追加した変数やcleanup登録は破棄しますが、実行中のブロックの枠は残すため、その後の処理やブロック終了を続けられます。既に終了した内側のスコープや別の関数呼び出しのCheckpointへの`revert`は`InvalidContinuation`になります。実行位置も戻す場合は`resume`を使います。

Checkpointはheap、変数、仮想file、Iterator cursor、task状態等を含みます。保持するCheckpointが多いほど履歴が残ります。乱数状態は戻せますが、消費済み予算・記録済みのHost観測・確定済み外部作用は戻しません。branchやasync/libraryの内部からpublishすることには制限があります。

publish前に外部fileの変更等を検査します。適用途中の失敗は`PublishPartiallyApplied`です。複数file・directory・streamの一括atomic性は保証しません。成功確認済みの操作と失敗phaseを調べ、外部状態を確認して判断します。同じruntimeでの盲目的な再publishは拒否します。

## cleanupとタスク

`defer`はscope終了・return・失敗時のcleanupに使います。クロージャー形式なら`defer ||->Unit{ ... };`です。FileHandleには`using`もあります。cleanupも失敗した場合、元のエラーを主診断に残し、cleanupエラーをcausesへ追加します。

```rewind
fn work()->Unit effects {output} {
    defer ||->Unit{Out.println("cleanup");};
    Out.println("work");
}
work();
publish;
```

`async fn`の呼び出しはTaskを作り、`await`は`Result<T,TaskError>`を返します。`spawn`、Channel、TaskGroup、キャンセル、logical timeoutを利用できます。これは決定的な協調schedulerであり、OS threadの並列実行を意味しません。

```rewind
async fn compute(n:Int)->Int effects {} { return n*2; }
let task=spawn compute(21);
match await task {
    Ok(value)=>{assert_eq(value,42);},
    Err(_)=>{panic("task failed");}
}
```

失敗taskのDiagnosticにはsource・line・column・taskId・causes・waitGraphがあります。v1.1のCLI/record JSONには追加でframes/hintsが入ります。TaskErrorの型付き分類、cancelAndJoin、TaskGroup.join等を使って失敗やcleanup完了を確認します。logical timeoutはwall clockの秒数ではありません。

## 文字列と標準ライブラリ

Stringのbuiltin memberと`std.text`はindexの単位が異なる部分があります。

| 操作 | 単位・戻り値 |
|---|---|
| `s.byteLen()`、`s.find(...)` | byte長、byte offset |
| `s.charLen()`、`std.text.length(s)` | Unicode scalar数 |
| `s.utf16Len()` | UTF-16 code unit数 |
| `s.slice(a,b)` | byteの半開区間。UTF-8境界を検査しResultを返す |
| `std.text.slice(s,a,b)` | Unicode scalarの半開区間。StdError付きResult |
| `std.text.find(s,needle)` | Unicode scalar offsetのOption |
| `Bytes`のget/slice | byte index |

Unicode scalarは利用者が見た一文字（grapheme）と一致するとは限りません。codec・parse・collection等には各APIの入力容量があります。

```rewind
import std.text as text;
assert_eq(text.slice("a😀界",1,3),Ok("😀界"));
assert_eq("a😀界".slice(1,5),Ok("😀"));
let counts=Map<String,Int>();
counts.set("apple",3);
assert_eq(counts.get("apple"),Some(3));
```

標準ライブラリは34 moduleです。[ライブラリ一覧](../libraries/README.md)とSDKのmodule別API文書を参照してください。主な分類は次の通りです。

| 分類 | module |
|---|---|
| 値・変換 | option、result、error、text、bytes、number、bits、math、integer、modular |
| collection | collections、map、sort、sequence、heap、deque、bitset |
| range・graph | disjointSet、rollbackSet、fenwick、segment、range、graph |
| 解析・入出力補助 | json、args、config、path、scanner、stream、csv |
| 数値・文字列処理 | matrix、dp、stringSearch |

mutable collection/parserの内部fieldはprivateです。factory・accessorを使ってください。sort.tryStableやsegment.tryUpdatedは、新しい結果を構築することでcallback失敗時にも入力を保持します。任意のin-place API全体を自動transaction化する保証はありません。

## エラーの調べ方

v1.1のtext診断はエラーコード・発生位置・内側から外側へのcaller・対処ヒントを表示します。例えばゼロ除算は次の形式です。

```text
error[DivisionByZero]: division by zero
  --> main.rw:2:12
  at divide (main.rw:2:12)
  at <entry> (main.rw:4:1)
  help: Check the divisor before division or remainder.
```

位置は1始まりで、columnはUnicode scalar単位です。LSPへはUTF-16位置に変換します。callerは最大32件で、それ以上は省略を明示します。local値・引数・closureのcapture値をbacktraceへ追加しません。source-free成果物でも埋め込まれたsource名と位置が残ります。任意のnative内部処理や補助Engineの各関数呼び出しまで完全なstackを示すものではありません。

`--diagnostic-format json`はstderrへJSONを出し、stdoutのアプリ出力と分離します。トップレベルにexit_status/message/diagnostic/publish_failure/retryable/retry_hintがあります。Diagnosticの追加fieldはframes/hints/frames_truncatedです。古い記録では省略される場合があります。

| コード | 見直す点 |
|---|---|
| DivisionByZero | 除数 |
| IntegerOverflow | signed64の範囲。std.integerのchecked API |
| IndexOutOfBounds | 負のindex、len以上のindex |
| AssertionFailed、Panic | 明示したassert/panicと入力 |
| InvalidArguments | 引数の数・型。期待signatureと実際の型 |
| UnknownName | 名前、scope、import。近いbinding名には候補を表示 |
| MissingEffect、EffectMissing | effect宣言と実行時の許可 |
| ExecutionBudgetExceeded、NativeWorkBudgetExceeded | loop、入力サイズ、指定予算 |
| HistoryMemory、HistoryStorage | 保持Checkpoint、データ量 |
| TaskDeadlock | waitGraphとtask/channelの進行条件 |
| ExternalStateConflict | 外部fileの変更 |
| PublishPartiallyApplied | publish_failureのphase/appliedと実際の外部状態 |

記録を使うと入力・scheduler選択・失敗の位置を再現できます。replayはcompilerの同じ版で行います。replayは通常、記録された仮想公開を再現し、Host fileを勝手に上書きしません。secret入力は再注入が必要な場合があります。

## 実行予算と制限

`--steps N`、`--native-work N`、`--task-steps N`で計算量を制限します。step/nativeの既定は各1,000,000、task-stepの既定は100,000です。明示した累積予算をrevertやsource内のruntime設定で回復することはできません。

source内の`runtime`設定でexecutionSteps/historyMemory/historyStorage/spillThreshold等を指定できます。履歴は永続データ構造を共有し、VMはsafe pointでheapを回収します。logical work/allocation予算は物理CPU時間・RSSの厳密な計測ではありません。

Linuxではcompilerとruntimeへ既定2GiBのaddress-space上限を適用し、`--memory-mib`で指定できます。WindowsではこのOS上限を提供せず、`--memory-mib`を明示するとunsupportedエラーです。両OSともVMのlogical予算は利用できます。allocator/OSの強制終了をResultとして回復する保証はありません。

sourceは1MiB/module、module数256などのcompiler上限があります。公開artifact/record形式のcompiler版をまたぐ互換性は保証しないため、アップグレード時は再生成してください。プロセス実行、JVM互換、任意の外部作用の巻き戻しは提供していません。HTTP/HTTPS は明示的な外部領域と権限で利用できます。

## GUI

`std.gui` はネイティブの単一 canvas を提供します。`gui` effect を明示許可し、scene を present した後 publish して表示します。イベントループ、Undo、入力 replay、各 OS の要件は [GUI guide](gui.md) を参照してください。

## 即時外部操作（1.5）

`external { ... }` は外部操作の結果を記録し、同じ checkpoint に戻った際は再利用します。`external fresh { ... }` は新しい操作です。結果は巻き戻さず、通常の変数と読取り位置だけを戻します。region の終了は publish ではありません。

v1.6 では async task も active branch 外で外部領域を使用できます。領域内の checkpoint / publish、領域を抜ける制御フロー、await / task switching は拒否します。通信を領域内で送信し、領域を出てから await します。`std.external.millis` は `external,clock`、`std.http` の送信は `external,network,tasks` の明示許可が必要です。DB はまだ利用できません。例・容量・失敗は [1.5仕様](REWIND_v1.5.md) と [1.6仕様](REWIND_v1.6.md) を参照してください。

v1.6 の main task は子 Task が動作中でも publish できます。その時点の仮想出力・file・GUI の差分を確定し、未完了の通信を完了扱いにしません。`task.isDone()` は待機せず完了を確認します。`Task.timeout` は従来どおり論理 step の上限です。HTTP の実時間上限は request の deadlineMillis / timeoutMillis で指定します。

### 逐次 HTTP と接続の所有権（1.6.1）

`std.http.download(Request)` は `Task<Result<HttpDownload,HttpError>>`、`read(&mut connection,maxBytes)` は `Task<Result<Option<Bytes>,HttpError>>`。EOF は `None`。`upload(Request,maxUploadBytes)` で `HttpUpload` を開き、`write(&mut connection,Bytes)` と `finish(&mut connection)` で送信・応答取得を行います。呼出しは external 内、await は外です。接続を取り出す際は `match move result` 等で所有権を移します。通常の `match result` は接続の共有借用なので、書込みや mutable borrow はできません。

接続はスコープ終了で閉じます。`close` / `closeUpload` は明示的な非同期 close。`using connection=move owner;` も使えます。`HttpDownload` / `HttpUpload` は Send、Share ではなく freeze できません。接続を含む Task の完了値は一度だけ取り出せます。ignore した接続結果は解放されます。checkpoint は token と所有情報を復元し、閉じた実接続は復活させません。詳しい制限は [1.6.1仕様](REWIND_v1.6.1.md) を参照してください。

## PostgreSQL adapter（1.7.1）

`std.db.credentials` は `Secret<String>` の接続文字列を immutable な opaque alias に登録し、`std.db.postgres` は verified TLS 接続の hot Task を返す。これらは `external` 内で呼び、Task の await は領域外で行う。SQLite と同じ affine な DbConnection / DbStatement / DbCursor と独立した DB transaction API を使う。column の型と DB error の SQLSTATE、期限・切断・cleanup の契約は [PostgreSQL 仕様](REWIND_v1.7.1.md) を参照。

## 型付き数値配列（1.8.0）

`import std.numeric as numeric;` で FloatArray / IntArray を操作する。rank / shape / stride を持つ native storage で、`zerosFloat` / `fromFloat`、`getFloat` / `withFloat`、reshape / transpose / slice / 明示 broadcast、vector 演算、matmul、LU solve、sum / mean / variance を提供する。Int 版もある。各関数は `Result<…,StdError>`。with は新しい所有値を返すので変数へ代入する。view は生成時の値を保持し、broadcast は read-only。VM の commit / revert、freeze / thaw、Task、artifact / replay に対応する。詳細なサイズ・誤差・費用は [数値仕様](REWIND_v1.8.md)、実行例は SDK の `examples/numeric` を参照。

### 線形代数・追加統計と分布（1.8.1）

`std.numeric` は列 pivot QR、QR による最小二乗、対称行列の固有値・固有ベクトル、共分散・相関・分位点・histogram、サンプルを保持しない OnlineMoments を提供する。結果の shape、rank、convergence と maximum work は [仕様](REWIND_v1.8.1.md) を参照。`std.distributions` は独立した random 効果の module で、uniform、normal、exponential、Bernoulli を VM の checkpointed generator から生成する。

### v1.8.2 BigInt

`std.bigint` の不変な native 値は64 bitを超える整数の正確な計算、bit operation、Map key、checkpoint に対応する。[容量・算術・JSON 契約](REWIND_v1.8.2.md)を参照。

### v1.8.3 Decimal

`std.decimal` は precision / scale / rounding を明示する exact base-ten 数値型。scalar 数値比較と VM の scale 保存を分け、`std.dbDecimal` で DB へ接続する。[契約](REWIND_v1.8.3.md)。

## 日時（v1.8.4）

`Instant` / `Duration` は不変の native 値です。`std.datetime` の変換・算術は pure で `Result` を返し、比較・Map key・freeze・task・checkpoint を利用できます。zone は明示し、DST overlap は Earlier / Later / Reject、gap はエラーです。`std.clock.now()` は `external,clock` effects と external region を要求します。詳細と DB の精度契約は [v1.8.4](REWIND_v1.8.4.md) を参照してください。

## Unicode（v1.8.5）

`std.unicode` の grapheme API は extended grapheme cluster、offsets は UTF-8 byte offset、従来の `std.text` は scalar 単位です。NFC / NFD / NFKC / NFKD と full default casing は明示的に呼び出す pure な Result API。String / Map の自動正規化は行いません。上限・費用・データ版は [v1.8.5](REWIND_v1.8.5.md) を参照してください。

## 初期履歴予算（v1.9.4）

`rewind run` の `--history-memory SIZE` / `--history-storage SIZE` / `--spill-threshold SIZE` は初期 Runtime 予算を設定します。byte 数・KiB・MiB・GiB に対応し、spill threshold の 0 は強制 spill に使えます。言語内の `runtime` 設定は実行時に適用されます。trace は初期予算を記録し、replay で復元します。OS の RSS 上限と累積 work quota は独立です。[費用と移行契約](REWIND_v1.9.4.md)。

## 最大流・二部マッチング（v1.9.5）

`std.flow` の residual network は VM 内の状態で、checkpoint / revert に対応します。`std.matching.maximum` は最大 matching と最小 vertex cover を返す pure な Result API です。[容量・計算量・失敗契約](REWIND_v1.9.5.md)。

## compact 記録（v1.9.6）

`--record-mode compact` は詳細ステップ履歴を保持せず、命令数・実行順序 digest・観測・最終状態を照合する replay を提供します。既定の debug 記録はステップ移動に対応します。[保証範囲](REWIND_v1.9.6.md)。

GUI の `gui.graphemeEditing(&mut view, true)` は結合文字・絵文字を cluster 単位で編集する。既定値は false。選択位置は scalar offset のままで、有効化時に境界へ切り上げ、有効化後は cluster 内部の `setSelection` を拒否する。モードも checkpoint の対象になる。[v1.9.7](REWIND_v1.9.7.md) と SDK の gui-grapheme 例を参照。

v1.9.8 では Share 制約付き generic 関数の再帰呼出しに対する effect 解析を修正した。標準ライブラリの lazySegment / trie / suffix / geometry は [各版の契約](REWIND_v1.9.8.md) を参照。

v1.9.9 の std.guiWindows は名前付きの複数 native 画面、入力振り分けと source-free replay を提供する。[GUI 契約](REWIND_v1.9.9.md)を参照。

v1.9.10 の `std.task.yieldNow()` は追加 Task を作らず明示的に VM task を切り替える。`std.numericAsync.dot` / `matmul` は native chunk の間で切り替える async 関数で、Task<Result<T,StdError>> を作る。数値失敗は内側、キャンセル等は await の外側 Result に返る。[契約](REWIND_v1.9.10.md)を参照。

`std.fft` の複素 transform / linear convolution、`std.sparse` の COO → CSR / matvec、`std.sparseAsync` の共役勾配 task は native 配列を使う。`std.numeric.scale` / `norm2` も提供する。サイズ、精度、SPD の前提、同期 kernel とタスク handoff の範囲は [v1.9.15](REWIND_v1.9.15.md) を参照。

## v1.9.16: native-array differentiation / optimizer / model

`std.autodiff` は VM-owned Tape と private Node で pure な数値演算を記録する。one-element loss から `backward(&tape,loss)` し、`gradient(&gradients,node)` の Option<FloatArray> を読む。constant / 未到達値は None。matmul、同形 binary、明示 broadcast、reshape / transpose、sum / mean、activation を native pages で計算する。

`std.optimize.sgd` と `adam/adamStep` は named native weights の更新と moment の checkpoint 復元に対応する。`std.models` は canonical checksum 付き binary codec と fileRead/fileWrite を明示要求する save/load を提供する。save は deferred write で、物理 file は publish 後に確定する。32 MiB container /128 parameter 上限と fatal VM budgets、同期 kernel、中間 gradient の保持は[仕様](REWIND_v1.9.16.md)を参照。[実行例](../examples/model-training/main.rw)。

`std.numeric.affine` は scale と offset の一括演算、`activation` は relu / sigmoid、`sumToShape` は explicit broadcast の元 shape への縮約を行う。fatal work/memory budgets と有限値・shape 契約を維持する。

## v1.9.17: module globals と optimizer

imported function は entry の無関係な global variable を裸の名前で参照できない。module 内の constant と明示した import は維持し、local に出ていた誤った shadow warning を解消する。旧 language mode の判定は維持する。SGD / Adam の signature は同じで、native pass により formula ごとの一時配列を省く。[仕様](REWIND_v1.9.17.md)。

1.9.18 は `std.task.selectReady<A,B>` / `Task.selectReady` による result を消費しない型の異なる Task の待ち合わせと、cursor / replay を維持した GUI 空 poll の記録圧縮を追加する。[契約と残る作業](REWIND_v1.9.18.md)を参照する。

1.9.19 は `std.guiWindows.nextEventAnyAsync` と native GUI / HTTP / SQLite の最小統合を追加する。主 Task の制約・待機中の協調動作・入力の cancellation・terminal error の journal・source-free / disconnected replay を検証する。[契約と残る作業](REWIND_v1.9.19.md)を参照する。

1.9.20 は Task の実行予算カウンタを checkpoint と共有し、最後の保持先がなくなれば回収する。累積予算と新規 Task の独立性を維持し、profile の current Task 表と全体合計を区別する。[契約と残る作業](REWIND_v1.9.20.md)を参照する。

1.9.21 は明示的な `external live` と `live` effect を追加する。新しい呼出しは物理操作を実行し、保持された既存 Task は復元しても同じ結果を返す。結果は Task / checkpoint の寿命に合わせて解放し、通常の記録付き操作は維持する。完全な record / replay / inspect は実行前に拒否する。[契約と検証](REWIND_v1.9.21.md)を参照する。

1.9.22 は `std.guiWindows.nextEventAnyLiveAsync` を追加する。既存の記録付き GUI 入力と区別し、保持された Task の復元、新規入力、main thread、キャンセル、結果の寿命を検証する。[契約と残る作業](REWIND_v1.9.22.md)を参照する。

## TCP（v1.9.23）

`std.tcp.connect` で `TcpSocket` を取得し、`read`, `write`, `shutdownWrite`, `close` を external region で Task として作成する。await は region 外で行う。必要な permission は `external,network,tasks`、live 方式は追加で `live`。平文 TCP であり、TLS は含まない。read の短い chunk / EOF、送信の確認済み量、非復元の接続寿命と例は [TCP 仕様](REWIND_v1.9.23.md)を参照する。

## TCP TLS（v1.9.24）

`std.tcp.connectTls` は system trust、`configuredTls` は明示的な PEM roots を使う。返り値は `Task<Result<TcpSocket,TcpError>>`、既存の read / write / shutdownWrite / close と組み合わせる。verify を無効にするオプションはない。詳細は [TLS 仕様](REWIND_v1.9.24.md)を参照する。


## HTTP server / router（v1.9.25）

`std.httpServer.listen` / `configured` で listener を取得し、`next` で affine な要求を待ち、`respond` で一回だけ応答を queue する。factory は external region、await はその外で行う。`external live` も利用できる。`std.httpRouter.find` は method / URI path の完全一致により最初の Route を返す。物理応答・接続寿命・size / concurrency の上限は [server 仕様](REWIND_v1.9.25.md) と [例](../examples/http-server/README.md) を参照する。


## 協調的 FFT（v1.9.26）

`std.fftAsync.transform` は入力配列を共有する pure な cold Task を返す。初期化以後は最大 4096 個の処理ごとに実行権を渡す。既存の同期 FFT と数学的な条件・逆変換の scale を共有し、cancel と checkpoint は VM 内の計算状態に適用する。[仕様](REWIND_v1.9.26.md) と [例](../examples/fft-async/README.md) を参照する。

## 協調的 CSR（v1.9.27）

`std.sparseAsync.matvec(matrix,right)` は pure な cold Task を返す。CSR の検証と計算を最大4096項目ごとに区切り、空行と長い1行も分割する。共役勾配法内の CSR 積も同じ処理を使う。1.9.31 から vector 演算・norm・dot も同じ Task 内で分割する。[仕様](REWIND_v1.9.27.md) と [例](../examples/sparse-async/README.md) を参照する。

## 名前解決の資源上限（v1.9.28）

HTTP / TCP / PostgreSQL の名前解決は process 全体で最大8件を共有する。cancel や deadline でも OS の処理が終了するまで枠を返さず、枠不足は typed failure を返す。数値 IP アドレスはこの枠を使わない。外部の名前解決は `revert` で取り消さず、記録 replay は名前解決を実行しない。[仕様](REWIND_v1.9.28.md)を参照する。

`std.httpServer.tlsCredential` と `configuredTls` は、秘密鍵を記録しない TLS HTTP server を提供する。[鍵登録・予算・期限・非復元の通信](REWIND_v1.9.29.md)を参照する。

`std.httpServer.bearerCredential` / `configuredTlsAuthenticated` は Authorization を native transport 内で検証して除去する。[契約](REWIND_v1.9.30.md)を参照する。

`std.numericAsync.scale` / `zipFloat` / `norm2` は最大4096値ごとの pure Task である。任意 rank と view、入力を変更しない失敗、途中 checkpoint と cancel の [契約](REWIND_v1.9.31.md)を参照する。

`rewind profile` の `runtime.native_resources` は現在の native 資源数を常に含む。操作履歴や credential 設定の量とは異なる。[GUI / HTTP / 両 DB の統合例](../examples/gui-data/README.md)と [1.9.32仕様](REWIND_v1.9.32.md)を参照する。

`std.numericAsync.solve(matrix,right,tolerance)` は入力コピー・LU 分解・後退代入を最大4096 work unit ごとに分割する pure Task である。正方 Float64 行列、rank-one rhs と有限非負 tolerance が必要で、同期版と同じ演算順序を維持する。[契約と予算の限界](REWIND_v1.9.33.md)を参照する。
