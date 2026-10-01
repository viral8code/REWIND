# REWIND v0.9.3 草案 — 主要な処理とアルゴリズム基盤

作成日: 2026-10-01。状態: **草案・未実装**。基準は [v0.9.2実装状況](v0.9.2-status.md)。大量の入力、典型的なデータ構造、複雑なアルゴリズムを REWIND で記述し、予算と計算量を説明できる状態へ進める。処理系の必要な補修と、source library として提供する処理を分ける。

## 1. すでに使えるもの

if/while/for、再帰、関数/closure、generic と trait bound、enum/match、Tuple、List/Map、borrow/move/Frozen、iterator、Checkpoint、record/replay は実装済み。Map は primitive key の順序付き storage と user Ord key に対応し、hash map と同じ計算量は仮定しない。

Int は checked signed64、Float は binary64。v0.9.2 で64-bit bit 演算・shift・popcount、radix変換、overflow を避ける mulMod、Unicode scalar/byte を分けた文字列 codec、ASCII token 分割を実装した。JSON/config/args、fold/find/count、Map getOr/contains、Option/Result adapter、署名付き SDK もある。これらを再実装項目にしない。

一方、sort/search、heap/deque、graph、数論などの標準 module はない。現時点で、小さい入力で動作することと大きい入力で要求する計算量を満たすことを同一視しない。

## 2. 最初に直す処理系のコスト

List/Map の HeapRef 読取り経路には `heap_get(...).cloned()` があり、get/len の前に collection 全体を複製する経路が残る。List 更新も Vec 複製を伴う。n回の単純な走査や O(log n) の更新を library に書いても、storage の費用で期待する計算量にならない場合がある。

| 必要な補修 | 受入条件 |
|---|---|
| read-only access | len/get が collection 全体を複製せず、取得値の大きさに対応する費用で動く。nested owner、Secret、Frozen、Ord 比較を保持 |
| 更新と履歴 | indexed update/add/remove をページ分割または構造共有で実装し、snapshot 未作成時と作成後を測定。Checkpoint が到達できる旧状態は保持 |
| allocation と予算 | step 数と native 処理量・allocation を別に測定し、巨大な pure native 処理で予算を迂回できない。上限超過は partial mutation を残さない |
| stack と呼出し | 深い DFS は library の明示 stack を初期方式とする。再帰上限とgeneric specialization 上限を診断で示す。無制限の再帰を約束しない |
| 型付き呼出し/cache | generic の型引数を branch/closure/import でも保持し、公開 signature・capture・依存変更で cache を無効化。特殊化のコード量を計測 |
| streaming input/output | 現在の readLine + tokens に加え、bounded byte chunk と cursor tokenizer、整数scanner、buffered writer。UTF-8境界/EOF/負数/overflow と観測順序を定義 |

既存の executionSteps と `--task-steps` を利用し、新しい予算指定機構を重複して作らない。現状の既定は execution 1,000,000、task 100,000 instruction、call frame 1,024。実行時/履歴/観測の予算を用途に応じて明示し、時間制限を instruction 数で保証できると説明しない。

## 3. 主要な source library

処理系で実現できるものは `.rw` を第一にする。native 化は計測で必要性が分かった primitive に限定する。各 module は公開 signature、所有権、効果、順序、計算量、失敗、予算を文書化する。

| 段階 | module | 主要な処理と条件 |
|---|---|---|
| A | sort / search | stable merge sort、in-place sort、lowerBound/upperBound/binarySearch。比較 callback と Ord、半開区間、重複、empty を統一 |
| A | sequence | reverse、rotate、unique、prefix sums、coordinate compression、permutation、two-pointer の利用例。新規 owner と borrowed mutation を区別 |
| A | deque / heap | ring-buffer deque、binary min/max heap、custom comparator。push/pop/peek/empty、capacity 予算。並べ替えの deterministic tie break |
| A | disjointSet | path compression と union by size、same/size。Checkpoint/revert と通常の圧縮の整合性を試験。独自 rollback DSU は別型 |
| A | integer / modular | gcd/lcm、powMod、inverse、extended gcd、prime sieve、factorization の bounded 基本形、combination。modulus>0、非可逆/overflow を Result |
| B | range | Fenwick、segment tree、lazy segment tree、sparse table。operation/identity/associativity の利用者契約、point/range update、境界 |
| B | graph | adjacency representation、BFS/iterative DFS、topological sort、Dijkstra、Bellman-Ford、SCC、MST、LCA。負辺/負閉路、未到達、距離加算 overflow |
| B | string | KMP/Z、rolling hash、trie。byte/scalar API を分離、hash collision を正確性の保証に使わない |
| B | matrix / dp | matrix product/power、bitset state、knapsack/LIS 等の例。dimension と allocation 予算、generic arithmetic の契約 |
| C | advanced graph | max flow/min-cost flow、matching、biconnected components。residual 更新、overflow、iteration 予算、検証可能な証拠 |
| C | transform / polynomial | NTT/convolution と条件付き FFT、polynomial 基本演算。root/modulus/size、丸め誤差と exact result の区別 |
| C | advanced string / geometry | suffix array/LCP、整数幾何/convex hull/intersection。文字列順序、外積 overflow、Float predicate の誤差を明記 |

A→B→C の順に依存を揃える。全項目を一度に compiler builtin に追加せず、v0.9.3 初回は A と、Fenwick/segment tree・BFS/Dijkstra の代表例までを完了単位とする。残る B/C は同じ草案で公開設計と検証例を管理し、未実装を明記する。

64-bit で足りない処理は現在の mulMod と checked arithmetic で可能な範囲を区切る。BigInt/UInt/wide integer は concrete な overflow 用例と memory budget が必要になった段階で型/APIを設計し、Float へ逃がさない。一般の graph weight/comparator/monoid の表現は既存 generic/closure で試し、具体例で成立しない部分だけ型検査を補修する。

## 4. 通常の開発で必要な周辺処理

args の bool/短い option/help と schema の対応、Map entries/update、path validation/join、CSV、構造化 diagnostics/logging を上記の基本 module と並行して整備する。既存 config.resolve と File.read/write、publish の競合検査を再実装項目にしない。

process 間 lock、単一文書の schema migration、power-loss durability は別の storage 契約が必要。原子性の範囲、観測/publish/rollback、lock の取得失敗と timeout を定義してから実装する。HTTP/DB/UI は capability を持つ別 adapter package とし、今回のアルゴリズム基盤の完了条件に含めない。

## 5. 検証と配布

- empty/single/duplicates/negative/extreme、無効な区間とcapacity、変換 overflow を契約テストにする。
- small input は素朴な参照実装と照合する。sort の順序/置換、heap invariant、DSU partition、range query、graph distance と証拠を property test で確認する。
- n/2n/4n の読み書きで heap clone・allocation・instruction・履歴量を測る。clone の二次増大を改善した証拠を残し、wall-clock だけを合否にしない。
- mutable data structure の Checkpoint/revert、snapshot 分岐、record/replay、source-free artifact、callback effect/borrow/Secret の拒否を確認する。
- streaming の chunk 境界・UTF-8分割・EOF・再実行を確認し、許可されていない Host I/O を行わない。
- std API snapshot の追加と互換差分、SDK の署名/導入/改変検知、文書と実行例を更新する。

処理系の計算量を先に説明できる状態へ直し、その上で library の計算量を保証する。未計測の高度な処理を「大規模入力対応済み」と表示しない。
