# REWIND v1.9.5 — 最大流と二部マッチング

## 標準 module

`std.flow` は directed integer capacity の residual network を VM 内に保持する。`create(vertices,capacity)`、`add(&mut network,from,to,capacity)`、`augment(&mut network,source,sink,limit)`、`edge(&network,id)`、`reachable(&network,source)`、`vertices`、`edges` を提供する。

vertex は 0-based、edge ID は forward edge の追加順。parallel / antiparallel edge、self loop、capacity 0 に対応する。capacity は非負 Int。source / sink は異なる有効 vertex、limit は非負 Int。augment は今回増えた流量を返し、後続の呼出しは現在の residual network を継続する。最大の limit は 9,223,372,036,854,775,807 で、合計が limit を超えないため flow の合計を wrapping させない。

Dinic の level graph と current-edge を使い、経路探索は明示的な List stack で行う。経路長に比例する再帰を使わない。worst-case の上界は O(V² E log pages)、network は O(V + E)、temporary storage は O(V)。永続 List の page lookup 費用を上界に含む。

reachable は現在の residual edge による到達可能性。最大流まで処理して sink が到達不能な場合、true / false の分割は最小カットになる。limit による途中停止だけでは最小カットを保証しない。edge は元の capacity と現在の flow を返す。checkpoint / revert は residual network も復元する。外部作用は行わない。

`std.matching.maximum(leftVertices,rightVertices,&pairs)` は `List<Tuple<Int,Int>>` の二部 edge を受け取り、最大 matching を返す。result の `size`、`left` / `right` の Option<Int> 対応表、`leftCover` / `rightCover` の Bool 表を利用できる。cover は最小 vertex cover で、true の総数が size と一致し、全 input edge の少なくとも一端を含む。重複 edge と empty partition にも対応する。入力は変更しない。

## 容量・失敗

flow は 262,144 vertices / 524,288 forward edges を上限とし、capacity は作成時に指定する。matching の network には partition と source / sink、partition ごとの端点 edge も含まれる。これらを含めて flow の上限を検査する。

不正な endpoint / negative capacity / 同一 source-sink 等は Range、容量不足は Capacity を Result の String として返す。add はこれらの検査後に mutation を始める。augment の argument 検査も mutation より前に行う。

有限 work / memory の admission は引き続き適用する。fatal budget failure が mutation の途中で起きた場合の rollback を Result として保証しない。意図的な大きい計算は `--steps` / `--native-work` / 履歴予算を調整し、必要な履歴を checkpoint に保持する。

## 検証

48 個の小さい有向 network を全列挙した cut と比較し、flow conservation、edge capacity、residual cut の一致を検査する。parallel / antiparallel / self-loop / zero capacity を含む。1,024 vertex の経路、partial augment、繰り返し呼出し、preflight error、checkpoint 復元も検証する。

2×3 の二部 graph 全 64 パターンで独立に列挙した最適 matching と比較し、対応表の対称性、edge の存在、cover の被覆と最適サイズを検査する。source-free artifact と checkpoint を含む trace replay、および配布 SDK の module 呼出しも検証する。

## 継続事項

lazy range、trie / suffix、transform / geometry、GUI、native kernel の公平性・キャンセル、履歴 metadata と trace の費用、v2.0 統合の受入条件は継続する。既定の debug trace は index 16 MiB に制限され、大きい純粋計算の全命令 debug 記録にも上限がある。この制限を最大流の能力不足や OS memory 上限と混同しない。大きい計算の記録方式は後続で改善する。
