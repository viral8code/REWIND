# REWIND v1.9.8

## generic の再帰呼出し

Share 等の制約を持つ generic 関数を解析する際、effect 解析の部分的な特殊化でも呼出し側の制約を維持する。従来拒否された `fn recurse<T:Share>(n:Int,v:T)->T` の再帰呼出しを受理する。制約を満たさない型を受理する変更ではなく、制約のない generic から Share 関数を呼ぶコードは引き続き拒否する。

## アルゴリズム

| module | API と契約 |
| --- | --- |
| lazySegment | Share な aggregate / tag、build・半開区間 update / query。combine の結合則 / identity、apply の分配則、compose(old,new) の適用順は利用側の責務。最大 1,048,576 要素。更新・query は O(log n) callback 回数で順序を保つ。query は tag を子へ伝播するため mutable borrow が必要。persistent List の page lookup は別に費用を持つ |
| trie | BytesTrie / TextTrie、exact lookup・挿入・削除・prefixCount。容量は root を含む node 数（1..1,048,576）。容量を事前検査し、削除した枝の ID を再利用する。Text API は String を UTF-8 byte の木へ格納し、有効な String prefix を検索する。Unicode 正規化は自動で行わない。byte API は NUL / 任意の byte を許す |
| suffix | byte suffix array / LCP / 検索区間。native radix doubling O(n log n)、Kasai LCP O(n)、scratch O(n)。最大 1 MiB。order / LCP は native paged IntArray で、セルごとの VM object を作らない。LCP[0]=0、LCP[i] は隣の前 suffix との長さ。空 pattern は n 個の非空 suffix に対応し、空入力では 0 個 |
| geometry | Int Point の orientation / onSegment / intersects / convexHull。差・積・determinant は BigInt で計算し、Int 全範囲に対応する。hull は重複除去、反時計回り、末尾に始点を繰り返さない。keepCollinear は境界点を保持し、完全に共線ならソートした distinct 点を返す。Float Vector の dot / cross / orientationFloat は有限値を検査し、向きの tolerance は determinant の絶対値単位で明示する |

VM 内の木・index・出力は checkpoint の対象になる。suffix の native work と scratch / 出力容量は実行前に検査する。各 module の Range / Capacity 等は文書化した事前検査を行うが、callback の panic や fatal budget による途中の mutation を自動的な transaction rollback とは扱わない。必要な回復点には checkpoint を使う。

実行例は [advanced-algorithms](../examples/advanced-algorithms/main.rw)。公開 API には失敗・費用契約を付け、署名付き SDK に source / API snapshot / 例を含める。

## 検証と継続項目

lazy affine tag を単純な配列と比較し、非可換 combine の順序を確認する。Trie は prefix・上書き・NUL・日本語・容量不変性・node 再利用・checkpoint を確認する。suffix は独立な suffix sort / LCP 計算と検索範囲を比較する。geometry は広い整数演算の参照と Int 両端、交差・退化・hull・tolerance を確認する。例は source-free artifact と debug / compact replay、展開後 SDK で検証する。

v1.9 の native kernel の公平性、GUI 拡充、履歴費用と v2.0 の統合は継続する。今回の Trie は UTF-8 byte storage であり、scalar ごとの node を持つ別実装を提供したとは扱わない。flow を使う matching は既存実装を利用し、Hopcroft–Karp へ変更したとは扱わない。
