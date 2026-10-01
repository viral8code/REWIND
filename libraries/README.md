# REWIND libraries

compiler 1.3.0 / `language = "1.3.0"` の標準ライブラリ。collection storage、ownership、型、Checkpoint は処理系が担い、上位の処理は REWIND source module として提供する。ライセンスは MIT。

| module | 公開 API | 契約 |
|---|---|---|
| json | parse, parseBytes, stringify, get, keys, kind, integer, floating, text, boolean, array | 不変 Json AST、pure、1 MiB/深さ64/65,536 node |
| config | Field, ConfigError, resolve | arguments > environment > file > fallback、schema 検証、128 field |
| args | ArgError, parse | `--name VALUE`、未知/重複/欠落拒否、256 token/128 name |
| collections | filter, map, any, all, fold, find, count | borrowed List、Share 要素、同期 pure Send+Share callback、入力順 |
| math | min, max, clamp, mulMod | signed Int、clamp 範囲検査、mulMod は正の modulus と i128 中間積 |
| option | valueOr, map | Share 値、pure callback |
| result | valueOr, map, mapError | Share の成功/失敗値、pure callback、未選択分岐を維持 |
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
