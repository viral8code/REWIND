# 第31章 構文と組込み操作の早見表

## 文の骨格

次は構文の対応表です。NAME、TYPE、EXPR等は説明用placeholderです。全体を実行するプログラムではありません。

| 用途 | 構文の形 |
|---|---|
| binding | let NAME:TYPE=EXPR; / var NAME=EXPR; |
| 代入 | NAME=EXPR; / NAME+=EXPR; |
| 条件 | if EXPR {...} else {...} |
| 反復 | while EXPR {...} / for NAME in START..END {...} |
| 分岐 | match EXPR {PATTERN=>{...},...} |
| 関数 | fn NAME(PARAMS)->TYPE effects {...} {...} |
| 非同期関数 | async fn NAME(PARAMS)->TYPE effects {...} {...} |
| 型 | struct NAME {...} / record NAME {...} / enum NAME {...} |
| alias | type NAME=TYPE; |
| generic | fn NAME<T:BOUND>(...)->TYPE effects {...} {...} |
| module | import std.number as number; |
| 公開 | pub fn / pub struct / pub record / pub enum / pub type |
| 共有借用 | &EXPR |
| 可変借用 | &mut EXPR |
| 所有権移動 | move EXPR |
| snapshot | freeze(EXPR) / thaw(EXPR) |
| 終了 | return EXPR; / break; / continue; |
| 履歴 | commit NAME; / revert NAME; / resume NAME; / drop NAME; |
| 隔離 | branch NAME {...} |
| 公開 | publish; |
| 即時外部 | external {...} / external fresh {...} / external live {...} |
| cleanup | defer EXPR; / using NAME=EXPR; |
| task | spawn EXPR / await EXPR |

文末にはセミコロン、blockには波括弧を使います。beginは予約名です。API文書の型表記には`Tuple<A,B>`等が現れますが、本文のtuple注釈では`(A,B)`も使います。関数型の効果表記`!{}`とcapture表記`~{Send+Share}`はcompilerのcanonicalな表記です。

## よく使う組込み関数

| 操作 | 意味・失敗 |
|---|---|
| assert(Bool) | falseでAssertionFailed |
| assert_eq(a,b) | 不一致でAssertionFailed |
| panic(String) | 通常復帰しない診断 |
| freeze(owner) | Frozen snapshotを作る。深さ・循環・資源制限あり |
| thaw(snapshot) | 独立した可変値を作る |
| secret(value) | Secretで包む |
| reveal(secret) | 秘密値を取り出す。出力・送信は利用者の責任 |

freezeで渡す所有権の規則は式と型に応じて検査されます。任意の資源や関数をfreeze可能と解釈しないでください。assertやpanicはResultを返す通常の入力検査APIではありません。

## String・数値のmember

| 呼出し | 戻り型 |
|---|---|
| String.byteLen / charLen / utf16Len | Int |
| String.encodeUtf8() | Bytes |
| Bytes.decodeUtf8() | Result<String,String> |
| String.contains / startsWith / endsWith(String) | Bool |
| String.find(String) | Option<Int>、byte offset |
| String.split(String) | List<String> |
| String.slice(Int,Int) | Result<String,String>、byte区間 |
| Bytes.slice(Int,Int) | Result<Bytes,String> |
| String.parseInt / parseFloat | Result<Int,String> / Result<Float,String> |
| Int.format / Float.format | String |
| Int.toFloatChecked() | Result<Float,String> |
| Float.toIntChecked() | Result<Int,String> |
| Float.isFinite / isNaN | Bool |

組込みmemberとstd.text/std.bytesのerror型が同じとは限りません。例えばBytes.decodeUtf8はString error、std.bytes.decodeはStdErrorです。変換を混ぜる関数ではEを合わせるか、明示的にerrorを変換します。

~~~rewind builtin-conversion
assert_eq("42".parseInt(),Ok(42));
assert_eq("abc".parseInt(),Err("InvalidIntOrOverflow"));
assert_eq(Bytes("hello").decodeUtf8(),Ok("hello"));
assert_eq(42.toFloatChecked(),Ok(42.0));
assert_eq(3.0.toIntChecked(),Ok(3));
assert_eq(1.5.isFinite(),true);
~~~

## コンソール・環境・file

| 呼出し | 戻り型と境界 |
|---|---|
| Out.println(value) / Err.println(value) | Unit、pending |
| Out.writeBytes(Bytes) | Unit、pending |
| Out.flush() / Err.flush() | Unit、publishとは区別する |
| In.readLine() | Option<String>、EOFはNone |
| In.readSecretLine() | Option<Secret<String>> |
| In.readChunk(Int) | Option<Bytes> |
| Args.all() | List<String>、起動引数 |
| Env.get(String) / getSecret(String) | Option<String> / Option<Secret<String>> |
| Locale.current() | String |
| Time.now() / Random.next() | Int、観測 / VM乱数 |
| File.readText(String) / readBytes(String) | Result<String,FileError> / Result<Bytes,FileError> |
| File.open(String) / openSnapshot(String) | FileHandle |
| File.writeText(String,String) / writeBytes(String,Bytes) | Unit、pending |
| Directory.entries(String) | List<String>、観測 |

Time.nowのIntをそのまま任意の単位のtimestampと見なさず、型付き日時にはstd.clock/std.datetimeを利用します。Fileにはcreate、delete、truncate、append、copy、move、Directoryにはcreate、delete、moveもあります。書込みはpublish境界を持ちます。読み取り失敗とfatal診断の扱いは操作ごとに確認します。

## Iterator・Result・Task

Iterator adapterのmap/filter/foldは同期callbackです。taskやpublishを伴うcallbackを渡せません。collectはList、enumerateはindexとのtuple、zipは二つの列のtupleを作ります。

Resultにはmap、mapErr、andThen、flattenがあります。andThenは同じerror型のResultを返すcallback、flattenは内外で同じerror型のResultを要求します。すべてのResultを任意のerror型へ自動統合するわけではありません。

async fnはcold Task、外部通信・DBの開始APIはhot Taskです。cold Taskはspawn/await等で開始します。Channelはcapacityを持ち、0ならrendezvousです。send/receiveをawaitし、完了値や閉鎖の失敗を処理します。

# 第32章 API編の読み方と保証範囲

## signatureから呼び方を読む

`pub fn getOr<K:Ord,V:Share>(values:&Map<K,V>,key:K,fallback:V)->V effects {}`なら、Mapを読む借用を一つ、キーとfallbackを渡し、Vを受け取ります。KにはOrd、VにはShareを要求し、外部効果はありません。

`&mut`の引数があれば変更を許す借用、借用のないowner引数なら所有権の受渡しを確認します。Task<Result<T,E>>を返す外部APIは、呼出し場所とawait場所、外側TaskErrorと内側Eを区別します。

canonicalな返却型と表示用宣言のaliasが違う場合があります。例えばGuiErrorはStdErrorへのaliasです。canonical型は解決後の契約、宣言は利用する名前と引数名を読むために載せています。

## モジュールの型と共通型

API編は全moduleの公開関数と、module固有の公開type/alias/traitを掲載します。各moduleへ共通に現れるbuiltin型は共通型付録へまとめ、同じ定義を86回繰り返しません。

機械生成schemaのconstructor fieldはexport検査用の情報です。DbConnection、HttpDownload、TcpSocket等のopaque native resourceについて、constructor fieldだけを根拠に任意の正当な接続を自作できるとは解釈しません。生成・所有・closeは専用APIとchecker/runtimeの契約に従います。内部のprivate fieldの型が表示されても、直接アクセスはできません。

## 費用と失敗

costとfailureは`@api`注釈の原文です。英語の原文も保つことで、容量、単位、失敗時の入力保持、fatal budgetとの区別を省略しません。注釈がない場合、独自の保証を補っていません。moduleの説明、signature、実装、本文の対応する章を一緒に読んでください。

実装表示でstdNumeric等のnative入口が現れる場合、その一行だけでnative内部の全処理量が説明されるわけではありません。公開注釈と本文の数値・外部・予算契約も適用されます。

## 現在の提供範囲

Linux x86_64 / Windows x64 SDK、ネイティブGUI、HTTP/HTTPS、TCP、HTTP server、SQLite/PostgreSQL、typed数値配列、linear algebra、統計、exact整数・十進数、日時、Unicode、regex、増分データ処理、自動微分・optimizer・model保存を提供します。

OS上の任意の外部作用をrevertすること、JVM互換、任意の子process実行、GPU/device I/O、全OS/architecture向けバイナリ、版をまたぐartifact/record形式の全面互換は提供範囲に含めません。外部作用は明示的な領域に置き、VM状態と区別します。

## 用語

| 用語 | 本書での意味 |
|---|---|
| owner | 可変値や資源の所有元 |
| binding | 名前と値の結び付き |
| snapshot | ある時点の不変状態 |
| checkpoint | 名前付きのVM保存状態 |
| pending | まだ外部へ確定していない操作 |
| publish | pending I/Oの確定 |
| observation / journal | Hostから得た観測と記録 |
| replay | 同じcompilerで記録を照合して再現 |
| COW | 共有中のデータを変更時に分離する方式 |
| affine | 資源を自由に複製せず所有・解放を管理する契約 |
| logical budget | VM契約上の仕事量・領域の上限 |
| fatal diagnostic | 通常のResultへ自動変換しない実行失敗 |
