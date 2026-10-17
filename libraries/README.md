# REWIND libraries

## codex/developで追加中のmodule

[構想から実装した機能](../docs/v2-next-implementation-index.md)を追加しています。snapshot/historyに加え、timeline、changes、historyGraph、parser、orderedMap、textIndex、lens、persistentSet、textEdit、schema、memo、lazyのsourceとtestに加え、既存csvStreamとschemaのrow連携、jsonStreamの非消費peek、BigInt/Decimal typed numeric API、nested Json materializerとschema-validation consumerを保存しました。compile・実行検証はユーザー方針により省略しており、公開版の利用可能APIとしては扱いません。

| module | 公開API | 契約 |
|---|---|---|
| snapshot | Snapshot<T>, capture, read, fork | 選択したdataのFrozen root、独立したfork、全VMや実行位置は保存しない |
| history | History<T>, HistoryError, create, save, get, remove, clear, names, contains, length | boundedな名前付きregistry、削除・名前再利用から取得済みsnapshotを分離、通常modelとしてrevert対象 |
| timeline | create, push, pushSnapshot, current, undo, redo, seek, get, clear | bounded data履歴、ring index、独立snapshot |
| changes | ChangeSet, diff, inverse, apply, compose, merge, previewRebase | String key Map、明示等値、衝突検査、変更列の段階snapshot |
| historyGraph | add, setHead, pin, previewPrune, prune, ancestors, mergeBases, path, matching | 有限DAG、親関係を保つ明示整理、複数merge base、通常VM model |
| parser | literal, satisfy, map, pair, orElse, cut, repeat, lookAhead, verify, andThen, spanned, chainLeft/Right | finite Bytes、原文範囲、pure callback、結合方向と上限 |
| orderedMap | create, put, get, remove, first/last, popFirst/Last, touch, keys, clear | 挿入順、既存key更新は位置保持、再挿入は末尾 |
| table | builder, addInts/Floats/Texts/Bools (+Nullable), build, select, dropColumns, rename, head, slice, batch, where, takeRows, mask*, isNull, with*, map*, cast, floatArray, intArray, withFloatArray, cell, ints/floats/texts/bools, nullCount | 型付き列（Int/Float/Text/Bool）とNULL maskを持つ不変table。最大100万行・256列。列の選択/改名/削除はデータを共有し、行操作は残す行×列を一度copyする。numeric kernelへはfloatArray/intArrayで渡す |
| textIndex | Source, ByteOffset, ScalarIndex, GraphemeIndex, create, byteAt/scalarAt/graphemeAt, sliceBytes/Scalars/Graphemes | 原文と境界表の保持、明示的な位置変換、別原文の範囲を拒否 |
| lens | create, get, set, modify, compose, optional, plan, apply, inverse, checkLaws | Share値のtyped path、検査付き更新、欠損保持、有限例の法則確認 |
| persistentSet | create, root, same, size, components, unite, uniteMany | 複数の不変版、union-by-size、既存native配列の共有page |
| textEdit | replace, mapByte/Scalar/Grapheme, inverse, chain, mapChainByte/Scalar/Grapheme, inverseChain | 原文保持、編集位置の明示policy、boundedな編集列と失敗step |
| schema | Unknown, Segment, Schema, Field, ValidationError, Decoder, any/boolean/number/decimal/timestamp/integer/text, nullable/array/field/object, validate, csvRow, mapped, decode | 入れ子JSON、exact Decimal JSON Text/CSV raw cellとRFC3339 instant text/CSVを検証し、Float丸めやnaive timestampを拒否、defaultと明示NULL、field名付きCSV row |
| csvStream | Reader, TypedRowError, reader, feed, next, nextTyped, finish, cancel, position | bounded queueのheader付きrow変換、Validation error時は行を保持、成功時だけadvance |
| jsonStream | Event, IntegerEventError, DecimalEventError, JsonValueError, SchemaEventError, reader, feed, peek, next, nextBigInt, nextDecimal, nextJson, nextValidated, finish, cancel, position | bounded JSON event queue。nextJson materializes one nested root value; nextValidated advances only after shared schema validation; incomplete/failure preserves queued events |
| db row mapping | ColumnMap, MappedBatch<T>, RowMapError<E>, SchemaRowError, SchemaBatchError, indexColumns, columnNumber, mapBatch, validateRow, validateBatch | 最大256列名を一度検査。pureなschema batch mapperは最大64行で、row error index付き。source DbBatch/cursorを変更しない |
| dbMigration | Migration, Applied, Plan, MigrationError, migration, checksum, plan, createTableSql, selectAppliedSql, recordSql | 利用者が書いたSQL command列を保持し、適用済み行との整合（接頭辞・checksum・順序・backend）を検証して未適用の末尾を返す。SQLの生成・分割・実行は行わない |
| plot | Series, Axis, Chart, Point, Segment, PlotError, series, axisOf, chart, project, polyline, scatter, svg, render | 数値配列を線形軸で画素座標へ写し、クリップと列ごとの極値保持で線分・点の数を上限内に縮約する。render以外は純粋で、描画はgui.lineのみ |
| dbTransaction | TxState, Transaction, TxError, Finished, begun, state, isActive, connectionReusable, requestCommit, requestRollback, completeCommit, completeRollback, close, finished | 物理BEGIN/COMMIT/ROLLBACKはstd.dbのtask。要求と応答の状態を追跡し、二重要求・終了後の要求を拒否、`DbError.phase`で失敗を分類。dropでcommitしない |
| memo | key, create, get, peek, put, resolve, invalidate, clear | 版付きkey、LRU、件数/重み上限、pureな結果とErrの保持 |
| lazy | create, isForced, peek, force, fork, clear | 明示的な遅延計算、成功snapshot、回復可能なErr、可変ownerの独立fork |
| url | UrlError, Param, Url, encodeComponent, decodeComponent, parseQuery, formatQuery, parse, format, display, pathSegments, resolve | URL・query・percent encodingのpure helper。成分はencodedのまま保持し、decodeは一度だけ、queryは順序と重複を保持、displayはuserinfoを隠す、errorは入力値を含まない。上限は8192 byte・128 param |
| issues | Issue, Report, Validated, IssueError, Collector, collector, add, addDetail, count, omittedCount, isEmpty, child, at, pathOf, fromValidation, finish, report, render | 複数の検証問題を上限付きで集める。上限（1..256）を超えた分は件数だけ数え、`Validated<T>`はValidかInvalidで部分値を保存へ流用できない。問題はpath・code・型記述だけで入力値を含まない。`std.schema`のerrorを取り込める |
| units | UnitError, Second, Milli, Nano, Byte, Scalar, Grapheme, Quantity, Id, of, amount, add, sub, less, equal, secondsToMilli, milliToNano, secondsToNano, milliFloorSeconds, nanoFloorMilli, idOf, text, sameId | 単位と不透明IDの型付きwrapper。`Quantity<Second>`と`Quantity<Milli>`、`Id<UserTag>`と`Id<OrderTag>`は型検査で混ざらず、中身はprivate。拡大変換は桁あふれを検査、縮小は明示した負の無限大方向の丸め。実行時費用は元のIntと同じ |
| httpHeaders | HeaderError, Header, Cookie, CookieSettings, lowerName, header, find, findAll, parseBlock, formatBlock, redacted, parseCookieHeader, setCookie | pureなheader/cookie helper。名前はtoken検査と小文字化、値はCR/LF/NULを拒否、繰返しheaderを保持、`redacted`はAuthorization/Cookie等の値を`***`にする。Set-Cookieの属性は検査（SameSite=NoneはSecure必須）。errorは入力値を含まない |
| retry | RetryError, Policy, Decision, policy, delay, jitter, decide | 明示retry policyの判定だけを行うpure module（待機も送信もしない）。上限付きの回数・指数backoff・全体deadline、`NotSent`は再試行可、応答errorは`retryResponses`かつ一時的なときだけ、`Unknown`は冪等と宣言したときだけ（通常の書込みは再送しない）。jitterは記録済みの乱数drawから決定的に作る |
| proptest | PropertyError, Failure, Outcome, draw, check | seed付きの決定的なproperty検査と縮小（pure）。`check`はseedから`Int`のlistを作り、失敗した最初の例を要素の削除と0（またはrange端）への縮小で小さくする。同じseedは同じ例と同じ結果。回数1..1000・長さ0..64・縮小0..2000。乱数はtest用でsecurity用ではない |
| logfmt | LogError, Field, Entry, Queue, field, entry, render, queue, push, dropped, drain | 構造化ログ行のpureな生成と上限付きqueue。1 entryは必ず1行（改行・制御文字・引用符を逃がす）、値は256 byteで切り、secret指定や認証情報らしい名前（password/token/authorization/cookie等）は`***`。queueは満杯なら新しい行を捨てて件数を数え、drainが`logs dropped`行で報告する。出力先への書込みは呼出し側 |
| failures | FixtureError, Script, fixtureDbNotSent, fixtureDbDeadline, fixtureDbDisconnected, fixtureDbRejected, fixtureDbCleanup, fixtureHttpNotSent, fixtureHttpDeadline, fixtureHttpStatus, fixtureHttpTruncated, script, nextResult, remaining | application test用の失敗fixture。実backendと同じ型・code・phaseのDbError/HttpError値と、用意した結果を順に返すScriptを作る。接続もnetworkも使わず、使い切ると`FixtureExhausted`で止まり実物へfallbackしない。名前に`fixture`を含め本番コードから誤用しにくくした |
| guiField | new, render, dispatch, setError, clearError, text, error, cursor, anchor | 単一行入力のVM状態、renderはI/Oなし、scopeごとのid、caretとvalidation errorを保持、focusは呼び出し側 |




[実行例](../examples/data-history.rw)と[型判定の変更](../docs/v2-next-frozen-bounds-implementation.md)を参照してください。

## 既存ライブラリと配布の記録

compiler 1.9.62 / `language = "1.9.62"` の標準ライブラリ。collection storage、ownership、型、Checkpoint は処理系が担い、上位の処理は REWIND source module として提供する。ライセンスは MIT。

| module | 公開 API | 契約 |
|---|---|---|
| httpServer | listen, configured, next, respond, close, closeRequest | Hyper HTTP/1.1、bounded listener / queue / body、recorded / live、affine handles、物理応答は非復元 |
| httpRouter | Route, find, allows, RouterError, Param, Router, Resolution, compile, resolve | pure な method / encoded URI path の完全一致（`find`、先勝ち、wildcard method）と、`compile`/`resolve`によるparameter付きrouter。`{name}`は1 segment、末尾の`{*name}`は残り全部を一度だけdecodeし、左のsegmentから literal > `{name}` > `{*name}` の優先で決め、同じmethodと形のrouteは`compile`で拒否する。`Found`/`MethodNotAllowed`（許可methodを整列）/`NotFound`/`BadPath`を返し、HEADはGET routeに、OPTIONSは自動応答しない |
| tcp | connect, connectTls, configuredTls, read, write, shutdownWrite, close | bounded raw TCP / verified TLS、binary chunk / EOF、recorded / live、非復元の接続寿命 |
| autodiff | Tape, Node, Gradients, parameter/constant, binary/unary, matmul, transpose, reshape, broadcast, sum/mean, backward/gradient | native Float64 buffers、private graph handles、COW checkpoint、同期 reverse-mode |
| optimize | sgd, Adam, adam, adamStep, steps | native moment、bias correction、typed error atomicity、checkpoint |
| models | Model, create, get, weights, encode, decode, save, load | bounded canonical binary、checksum、native pages、fileRead/fileWrite と publish 境界 |
| fftAsync | transform | cold pure Task、4096 個までの分割、初期化から handoff、COW scratch / checkpoint、cancel |
| fft | Spectrum, transform, convolve | native complex radix-2 / Float64 linear convolution、2^20、O(n log n)、pure / COW / replay |
| sparse | Matrix, create, matvec | canonical CSR、stable COO duplicate sum、borrowed pages、dense 展開なし |
| sparseAsync | matvec, SolveResult, conjugateGradient | CSR を最大4096項目ずつ処理、反復内の handoff、実 residual、typed exhaustion / cancel |
| lazySegment | LazySegment, build, update, query, length | Share aggregate / tag、半開区間、順序を保つ O(log n) callback、VM checkpoint |
| trie | BytesTrie, TextTrie, create/createText, get/textGet, insert/textInsert, remove/textRemove, prefixCount/textPrefixCount, size/textSize, nodes/textNodes | 指定 node 容量、UTF-8 と任意 byte の API を区別、削除した枝を再利用 |
| suffix | SuffixIndex, build, order, lcp, find, length | byte suffix array / LCP、1 MiB、O(n log n) native 構築、paged IntArray |
| geometry | Point, Vector, orientation, onSegment, intersects, convexHull, dot, cross, orientationFloat | Int 全範囲の exact predicate、Float tolerance 明示 |

| json | parse, parseBytes, stringify, get, keys, kind, integer, floating, text, boolean, array | 不変 Json AST、pure、1 MiB/深さ64/65,536 node |
| config | Field, ConfigError, resolve | arguments > environment > file > fallback、schema 検証、128 field |
| args | ArgError, parse | `--name VALUE`、未知/重複/欠落拒否、256 token/128 name |
| collections | filter, map, any, all, fold, find, count, tryMap, tryFold, partition, Split, collectUpTo, Bounded | borrowed List、Share 要素、同期 pure Send+Share callback、入力順。`tryMap`/`tryFold`は最初のErrで止まり入力を消費しない（途中の出力は捨てる）、`partition`は順序を保って二つに分ける、`collectUpTo`は上限までだけ複製して切詰めの有無を返す |
| math | min, max, clamp, mulMod | signed Int、clamp 範囲検査、mulMod は正の modulus と i128 中間積 |
| option | valueOr, map | Share 値、pure callback |
| result | valueOr, map, mapError, mapOwned, thenOwned, mapErrorOwned | valueOr/map/mapError は Share の値用。`*Owned` は Result を消費し、非Share の owner（List、接続など）を複製せず一度だけ次へ渡す |
| map | getOr, contains | borrowed Map、Ord key、Share value、key equality は既存 Map と同じ |
| text | length, trim, split, tokens, slice, find | Unicode scalar index、slice は半開区間、split は空要素保持、tokens は ASCII whitespace |
| bytes | length, encode, decode, get, slice | UTF-8 codec、byte index、get は0..255、slice は半開区間 |
| number | integer, decimal, floating, formatInt, formatFloat | signed64、radix2..36、小文字 digit、有限 binary64 |
| bits | and, or, xor, not, count, shiftLeft, shiftRight, shiftUnsigned | 64-bit 二の補数、shift0..63、left は wrapping、right は符号拡張/zero fill を区別 |

## SDK から導入

配布者の公開鍵を別経路で確認し、development project に導入する。

```sh
rewind sdk-verify --sdk /path/to/sdk --public-key PUBLIC_KEY_HEX
rewind sdk-install --root /path/to/project --sdk /path/to/sdk --public-key PUBLIC_KEY_HEX
rewind check --root /path/to/project
```

```rewind
import std.number as number;
assert_eq(number.integer("-ff",16),Ok(-255));
```

署名付き std source を project の vendor に固定する。ネットワーク、自動 trust、global import、SDK 自動更新は使わない。旧 language を使う既存 project はその版の module source を保持する。`examples/v091/lib` は v0.9.1 の固定例である。

## 位置・失敗・予算

codec/parse/format/slice/shift/mulMod は `Result<...,StdError>`。StdError は immutable record `{code:String,offset:Int}`。入力値をエラーへ含めない。code は Utf8 / Range / Radix / InvalidNumber / NumberRange / EmptyDelimiter / ShiftRange / Modulus / Limit。offset は UTF-8 decode の不正部分開始 byte、それ以外は0。整数は空白や prefix を受理せず、符号と radix の digit を解析する。Float は Rust の有限数解析/最短 round-trip 表示に従い、decimal 精度や JSON canonicalization を保証しない。

text trim/split/tokens/slice、UTF-8 encode/decode、数値 parse の入力は1 MiBまで。split/tokens の出力は65,536要素・合計1 MiBまで。bytes slice の出力は1 MiBまで。find は1 MiBを超える入力で通常の実行エラーとなり、成功時は Option<Int>。length と bytes get はサイズ上限を追加せず既存の値を読む。pure は Host effect がないことを意味し、計算量や heap allocation が無制限という意味ではない。Secret は明示 reveal なしに通常の型へ渡せない。

List/Map の更新、snapshot、Checkpoint、replay は既存 runtime の契約に従う。callback は別 task へ spawn しない。新しい List を返す処理は caller が所有する。mutable payload の Result/Option は move/borrow/freeze を必要とする。

## 開発と検証

```sh
rewind update --root libraries/std
rewind test --root libraries/std
rewind doc libraries/std/text.rw --root libraries/std --output /tmp/text-api.md
rewind api-snapshot --root libraries/std --output /tmp/std-api.json
```

10契約テストで codec、Unicode index、bit/number 境界、Result の両分岐、collections、設定優先順位と引数失敗を検証する。SDK の組立てもこのテストを実行する。Host I/O は app で許可・観測・publish を明示する。SDK の例は sum と JSON 保存 app。配布手順は [v0.9.2仕様](../docs/REWIND_v0.9.2.md)、主要なアルゴリズムと性能改善は [v0.9.3草案](../docs/REWIND_v0.9.3.md) を参照。

## v0.9.3 アルゴリズムと周辺API

| module | 主要API | 契約 |
|---|---|---|
| sort/search | stable, inPlace, integers / lowerBound, upperBound, binary | Share要素、pure Send+Share comparator、stableは入力を変更しない。searchは同じ順序で整列済みの入力 |
| sequence | reverse, rotate, unique, compressed, prefix, nextPermutation | rotateは左向き・負数対応、uniqueは連続重複、compressedはIntの昇順distinct、prefixは初期0を含む。permutationの最後は昇順へ戻す |
| heap | push, peek, pop | borrowed List<T>、同じcomparatorを継続使用、pushは明示capacity、equal orderはcomparatorでtie-break |
| deque | create, pushBack/Front, popBack/Front, length | ring buffer、1..65,536slot、emptyはNone、fullはErr(Capacity) |
| disjointSet | create, root, unite, same, size | 0..65,536要素、path compression+union by size、範囲を検査 |
| integer/modular | add, multiply, gcd, lcm, extendedGcd, primes, factors, combination / normalize, add, multiply, power, inverse | signed64、overflowはResult、modulus正、inverseはmodulus>1とgcd=1。extendedGcdは非負入力・中間演算もsigned64 |
| fenwick | create, add, prefix, range | 0..65,535要素、prefix[0,end)、range[start,end)、updateはoverflowをpreflight |
| segment | build, set, query | Share要素、0..32,768要素、associative combineとidentityをcallerが保証。順序を保持する半開区間query |
| graph | create, add, bfs, dfs, dijkstra | directed adjacency、0..65,536vertex/edge。bfs未到達=-1、dijkstra未到達=None、負辺/overflow拒否。adjacency traversalは挿入の逆順 |
| scanner | fromBytes, nextInt, nextToken | 1MiBまで、ASCII whitespace、signed64、nextTokenはBytes、error時のcursorは解析済み位置に進む。Host effectは持たない |
| path | relative, join | lexical relative path、空/dot/parent/absolute/backslash/colon/NULを拒否。symlink/root境界はHostが検査 |

argsにOptionSpec/parseOptions/helpを追加した。bool flagは"true"、short optionは1 scalar、required/duplicate/unknownを検査する。--name=value、option bundling、positional schemaは未対応。Map entries/updateはString keyとShare valueに限定する。

全algorithmはpure。Result errorは主にString code（Range/Capacity/Overflow/Modulus/NegativeWeight/NonInvertible等）。panicや予算超過は通常の実行失敗となる。prime sieve上限65,535、trial factorization入力1..10^12、combination反復上限65,536。graph distance加算もchecked。mutable構造の公開fieldを直接改変せず、同じmonoid/comparatorで操作する。

List/heapのstorage操作はO(log(max(1,n/64)))、writeは最大64slotを共有copyする。この費用を通常のalgorithm計算量へ掛ける。stable sortはO(n log n)比較、heapはO(log n)比較、binary searchはO(log n)比較、Fenwick/segmentはO(log n)演算、BFS/DFSはO(V+E)走査、DijkstraはO((V+E)log(E+1)) heap操作を土台とする。返すpayloadとcallbackの費用は別。DSUの圧縮はCheckpointで復元でき、通常の経路圧縮後の状態を維持する。Map更新とuser Ord Mapに同じstorage計算量を適用しない。

Generic factoryはselected importを使う：`import std.deque.{create as createDeque}; createDeque<Int>(4)`。Host inputはappで読み、Bytesへ変換してscannerへ渡す。chunkをまたぐ一般のstreaming decoderは未実装。SDKのshortest例を参照。

source libraryの契約テストは10件。詳細は[v0.9.3実装状況](../docs/v0.9.3-status.md)、次段階は[v0.9.4草案](../docs/REWIND_v0.9.4.md)。

## v0.9.4 streamingとMap

26番目のmodule `std.stream` はpureなchunk処理を提供する。`tokens(limit)` / `feed(&mut tokens,chunk,finish)` / `next` / `nextInt`、`utf8` / `decode`、`writer(capacity)` / `write` / `drain`を使う。失敗は`StreamError{code,position}`。tokenは最大65,536 byte、待機queueは1,024 token、writerは最大65,536 byte。UTF-8 decoderのchunkは65,532 byte以下、境界の未完文字は最大3 byte保持する。positionは当該chunk（UTF-8は前回のcarryを含む）または変換対象token内の位置であり、入力全体のoffsetではない。容量・UTF-8検査の失敗前に状態を変えないが、runtime budget失敗までatomicにする保証はない。

Host観測はapp側で`In.readChunk(limit)->Option<Bytes>`、出力は`Out.writeBytes(bytes)`と`publish`で明示する。EOFはNone、短いchunkを許し、revert時は記録済みbyteを再利用する。primitive keyのnative Mapはpersistent AVL treeでroot clone O(1)、更新のnode path O(log n)。payloadの生成・返却費用は別。user Ord keyのOrderedMapは従来の線形storageを維持する。

source libraryの契約テストは11件。詳細は[v0.9.4実装状況](../docs/v0.9.4-status.md)、継続計画は[v0.9.5草案](../docs/REWIND_v0.9.5.md)。

## v0.9.5〜v0.9.9

現在のSDKは33module。追加moduleはstringSearch（byte KMP/prefix/Z）、bitset（packed bits）、range（sparse minimum）、rollbackSet（rollback DSU）、csv（bounded UTF-8 CSV）、matrix（checked product/power）、dp（LIS/0-1 knapsack）。graphにtopological/Bellman-Ford/SCC/minimumForest/ancestors/lcaを追加した。modular.inverseは正のInt64全域のmodulusを扱う。各容量・失敗・計算量は[v0.9.6](../docs/v0.9.6-status.md)、[v0.9.7](../docs/v0.9.7-status.md)、[v0.9.8](../docs/v0.9.8-status.md)を参照。

標準契約テストは15件。heap回収とwork budgetは[v0.9.9](../docs/v0.9.9-status.md)。公開fieldの不変条件やphysical/native予算など、残る課題は[1.0準備草案](../docs/REWIND_v1.0.md)。

## 1.0の公開境界

mutable collection/parserの内部fieldとconstructorはprivateです。公開factoryとget/set/length等を使います。matrix.rows/columns、graph.vertices/edges、scanner.position等は読み取り用関数です。Forest等の公開結果fieldは利用できます。

error moduleは不変Error envelopeとFile/JSON/codec/Diagnosticからの変換を提供します。sort.tryStableとsegment.tryUpdatedはResultを返すpure callbackを受け取り、新しい結果を構築するので、callbackや予算の失敗時に元の入力を変更しません。既存in-place API全体のatomic性を追加するものではありません。費用・失敗契約をAPI snapshotにも含めます。

提供する35 moduleと保証範囲は[1.0仕様](../docs/REWIND_v1.0.md)を参照してください。0.xの内部field直接操作はfactory/accessorへ移行し、lock/artifact/recordを再生成します。

1.1では[言語リファレンス](../docs/language-reference.md)とWindows SDKを追加し、std.text.sliceのoffset表allocationを削除しています。公開APIの基準は1.0のsnapshotを維持します。

1.2では条件分岐内のCheckpoint復元と数値・文字列・コメントの表記を改善しています。既存の34module APIを維持します。

## std.gui（1.3）

ネイティブウィンドウ、ラベル、ボタン、チェックボックス、矩形、クリックとキー入力、publish による描画確定、入力 replay を提供します。`gui` effect の明示許可が必要です。[GUI guide](../docs/gui.md) と [counter sample](../examples/gui/README.md) を参照してください。

## std.external（1.5）

`millis` は external 領域での UTC milliseconds 観測。操作位置ごとに結果を記録し、revert 後に再利用します。`external,clock` を明示許可してください。[1.5仕様](../docs/REWIND_v1.5.md) を参照。

## std.http（1.6）

`get` / `request` / `configured` / `send` は external 領域で送信して Task を返します。await は領域外で行います。verified TLS、bounded body、複数 header、CA、credential alias に対応し、revert / replay で再送しません。権限・上限・失敗区分は [1.6仕様](../docs/REWIND_v1.6.md) を参照。

1.6.1 は `download / read / close` と `upload / write / finish / closeUpload` を追加する。接続は Send / 非 Share、所有スコープで解放する。Request.body は upload 時に空 Bytes。chunk は read 1〜65,536 / write 0〜65,536 bytes、累積 1 GiB、upload 応答は 4 MiB。16 MiB の外部記録上限は別に適用する。[契約](../docs/REWIND_v1.6.1.md)、[逐次受信例](../examples/http-stream/main.rw)を参照。

## std.db（1.7）

SQLite の接続、パラメータ束縛、prepared statement、逐次 cursor、batch 実行、独立した DB transaction を提供する。呼出しは external 領域、await は領域外。`external,db,tasks` の明示許可が必要。DB 書込みは VM revert で取り消さない。[契約](../docs/REWIND_v1.7.md)、[例](../examples/database/README.md)を参照。PostgreSQL の実接続・SCRAM・verified TLS・server cursor は [1.7.1](../docs/REWIND_v1.7.1.md) と [例](../examples/postgres/README.md)を参照。

## std.numeric

FloatArray / IntArray の生成、所有値の COW 更新、view、明示 broadcast、scalar math、vector / matrix product、部分 pivot LU solve、補償和・mean / variance を提供する。全要素を VM List に変換せず native kernel を使える。数値配列は VM 内の巻き戻せる値であり、DB 等の外部作用とは区別する。[仕様](../docs/REWIND_v1.8.md)、[例](../examples/numeric/README.md)を参照。

QR / 最小二乗 / 対称固有値、共分散・分位点・histogram と online moments は [1.8.1](../docs/REWIND_v1.8.1.md) を参照。`std.distributions` は checkpointed generator の uniform / normal / exponential / Bernoulli sampling を提供し、random 効果を持つ。

## v1.8.2 exact integers

`std.bigint` の native BigInt は参照共有され、parse / format、整数・bit算術、Mapとexact JSON string を提供する。[契約](../docs/REWIND_v1.8.2.md)。Decimal・DB変換は次の数値型patchで追加する。

## v1.8.3 Decimal と exact DB

`std.decimal` / `std.dbDecimal` は explicit rounding、JSON、PostgreSQL NUMERIC と SQLite BLOB の exact 変換を提供する。[契約](../docs/REWIND_v1.8.3.md)。

`std.datetime` は整数ナノ秒の Instant / Duration、RFC3339、暦、IANA zone と明示的な DST policy を提供します。`std.clock.now()` の時計観測は external、`std.dbDatetime` は精度を検査した PostgreSQL / SQLite 変換です。仕様は [v1.8.4](../docs/REWIND_v1.8.4.md)。

`std.unicode` は明示的な Unicode 正規化、grapheme / word / sentence boundary と大小変換です。既存 scalar API と GUI cursor の単位は保ちます。[v1.8.5](../docs/REWIND_v1.8.5.md) を参照してください。

## 最大流・マッチング（v1.9.5）

`std.flow` は VM 内の residual network に iterative Dinic の増加処理を行います。`std.matching` は二部最大 matching と最小 vertex cover を返します。checkpoint で residual state を保持できます。[API・費用・容量](../docs/REWIND_v1.9.5.md)、[実行例](../examples/flow/main.rw)。

## std.guiWindows（1.9.9）

名前付きの複数ネイティブウィンドウ、入力の振り分け、公開済み画面の保持、source-free replay を提供する。[GUI guide](../docs/gui.md) と [例](../examples/gui-windows/README.md) を参照。旧 `std.gui` の単一ウィンドウと入力 cursor は独立している。

## std.task / std.numericAsync（1.9.11）

明示的な VM task handoff と、最大4096積和ごとに処理を渡す async 内積・行列積を提供する。入力は native storage を共有し、TaskError と数値 StdError を分ける。[契約](../docs/REWIND_v1.9.11.md)と [例](../examples/numeric-async/README.md)を参照。

1.9.11 は FloatArray / IntArray の共有 page の会計を改善し、task と checkpoint での重複計上を解消します。API と source module は継続します。[費用と保持の契約](../docs/REWIND_v1.9.11.md)を参照してください。

## フォーム・表（1.9.12）

`std.guiForm` は form内検証・共通schema検証・型付きrecord mapping、`std.guiTable` は frozen 行優先データの表示範囲描画を提供する。`std.gui` は focus / focused / setChecked / remove / style / textarea scroll の編集を追加した。`Layout` / `WeightedLayout` と `leaf` / `weighted` / `sized` / `row` / `column` で最大32階層のlayout treeを構成できる。`sized`はmain axisの最小・最大寸法を指定し、同じ値で固定できる。`applyLayout` は最大1024 nodeを走査し、全leaf IDとboundsを検証してから一括更新する。`splitRow` / `splitColumn` も維持し、`arrangeRow` / `arrangeColumn` はflatな配置に使える。いずれも logical View の変更は pure で、外部 surface への反映には present と publish を使う。[容量・失敗・費用契約](../docs/REWIND_v1.9.12.md)、[実行例](../examples/gui-controls/README.md)を参照する。

1.9.13 は `gui.scene` の一時 Map / Json 構築を直接の serializer へ置き換える。public API と JSON の値を維持し、事前の byte 上限 / native memory admission を行う。[費用・互換性・測定](../docs/REWIND_v1.9.13.md)を参照する。

1.9.14 は manifest / lock を保持した source-free `.rwc` の実行・replay に対応する。実行時に source / vendor の graph を開き直さず、effect と assets の検証は保持する。[契約](../docs/REWIND_v1.9.14.md)を参照。

勾配・model の基本契約は[1.9.16](../docs/REWIND_v1.9.16.md)、module scope / optimizer の改善は[1.9.17](../docs/REWIND_v1.9.17.md)を参照。

1.9.18 は `std.task.selectReady<A,B>` / `Task.selectReady` による result を消費しない型の異なる Task の待ち合わせと、cursor / replay を維持した GUI 空 poll の記録圧縮を追加する。[契約と残る作業](../docs/REWIND_v1.9.18.md)を参照する。

1.9.19 は `std.guiWindows.nextEventAnyAsync` と native GUI / HTTP / SQLite の最小統合を追加する。主 Task の制約・待機中の協調動作・入力の cancellation・terminal error の journal・source-free / disconnected replay を検証する。[契約と残る作業](../docs/REWIND_v1.9.19.md)を参照する。

Task の予算・profile の寿命は [v1.9.20](../docs/REWIND_v1.9.20.md) を参照してください。

名前解決は HTTP / TCP / PostgreSQL 共通の上限で処理する。cancel 後も OS の処理が終わるまで枠を保持し、再送や証明書検証の緩和は行わない。[1.9.28 の契約](../docs/REWIND_v1.9.28.md)を参照する。

TLS HTTP server の秘密鍵登録・接続予算・期限は [1.9.29 の契約](../docs/REWIND_v1.9.29.md)を参照する。

TLS HTTP server の認証、秘密ヘッダーの除去、native 401 と replay は [1.9.30 の契約](../docs/REWIND_v1.9.30.md)を参照する。

`numericIndex` (language 1.9.48) は型付き数値配列を単一の整数で読み書きします。`getFloat/Int` と `withFloat/Int` はビュー上の行優先順序、`lengthFloat/Int` は論理要素数です。座標 List を生成せず、broadcast の更新は NumericReadOnly、範囲外は NumericIndex になります。

`graphLarge`（language 1.9.49）は数値配列に隣接データを保持します。`fromEdges` は辺をまとめて構築し、`bfs` は到達しない頂点を -1 とする IntArray を返します。`numericRange.integers(start,step,length)` は途中の List を作らず整数配列を生成します。既存の `graph`、`numeric`、`numericIndex` の API は維持します。

`std.training` provides named gradient extraction, mean-square loss, sample-weighted batch-mean merging and global-norm clipping. See [the nonlinear training example](../examples/nonlinear-training/README.md).

`std.gui` / `std.guiWindows` のネイティブ text input は選択言語 1.9.52 で Ctrl+C / Ctrl+V / Ctrl+X に対応します。公開 scene の選択をコピーし、貼り付けを記録する text 入力として受け取ります。OS クリップボードの状態は巻き戻しません。[仕様と制限](../docs/REWIND_v1.9.52.md)、SDK の gui-clipboard 例を参照してください。

`std.numericTransformAsync` は選択言語 1.9.53 の協調的な単項数値変換・活性化関数です。既存の scalar 演算と精度・失敗契約を共有し、4096 要素以下の native step 間でタスクを切り替えます。autodiff.backward 自体は同期のままです。[契約と制限](../docs/REWIND_v1.9.53.md)、SDK の unary-async 例を参照してください。

`std.numericReduceAsync` は選択言語 1.9.55 の協調的な合計・平均・分散です。既存の Neumaier / Welford 順序を保ち、4096要素以下の native step 間でタスクを切り替えます。[契約と制限](../docs/REWIND_v1.9.55.md)。

`std.graphLargeAsync.bfs` は選択言語 1.9.56 の協調的なグラフ構築・幅優先探索です。IntArray の辺入力を共有し、検証・構築・探索・距離配列の仕上げを4096単位以下に分割します。最短距離（未到達は -1）、途中の巻き戻しと取消を扱います。[契約と制限](../docs/REWIND_v1.9.56.md)。

`std.numericShapeAsync` は選択言語 1.9.57 の協調的な論理形状変更と broadcast の軸の合計です。寸法 List を所有し、数値入力のページを共有します。[契約と制限](../docs/REWIND_v1.9.57.md)。

`std.autodiffAsync` は所有する Tape の逆伝播を協調実行します。既存の Node / Gradients、各微分の演算順序を保ち、履歴準備も分割します。前向き演算には `std.autodiffForwardAsync` の所有型Task APIも利用できます。`examples/autodiff-async` に checkpoint・取消を含む例があります。

`std.guiMenu` は無効化・チェック状態・キーボード移動・スクロールを持つ上限付きメニューです。`gui.bounds` / `graphemeMode` / `overlay` で既存の画面へ重ねます。ネイティブ入力・巻き戻し・再生の利用例は `examples/gui-menu` にあります。

`std.guiDialog.openFile` / `saveFile` は、明示的な external 領域で作成するネイティブファイル選択 Task です。ユーザーによるキャンセル、VM の巻き戻しと OS の画面の寿命を分け、選択だけではファイルを作成・変更しません。Linux はシステムの GTK 3、Windows は標準ダイアログを使います。`examples/gui-dialog` に利用例があります。

選択言語1.9.61では、GUIの公開した入力欄でネイティブIMEの変換途中を表示します。確定した文字だけが入力記録に入り、変換中のOS状態はcheckpointに含まれません。[IMEサンプル](../examples/gui-ime/README.md)と[v1.9.61仕様](../docs/REWIND_v1.9.61.md)を参照してください。

`std.autodiffForwardAsync` はparameter・constant・既存の二項／単項演算・行列積・形状変換・集約を分割実行し、`Tuple<Tape,Node>`を返します。同期APIと同じ識別値・勾配を保ちます。`std.numericIdentityAsync` は有限値検証と保存領域のkey準備を分割します。[契約と追加費用](../docs/REWIND_v1.9.62.md)、[復元と取消](../examples/autodiff-forward/README.md)、[native入力](../examples/forward-gui/README.md)。
