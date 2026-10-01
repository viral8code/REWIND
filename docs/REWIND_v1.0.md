# REWIND 1.0準備草案

2026-10-01。状態: **準備草案・未実装**。v0.9.9の次に安定版へ向けて解消する残課題。主要機能が増えたことを、全ての境界の保証が完成したことと混同しない。

## 安定版前の優先事項

1. module private field/opaque type、複数generic bound/where、期待型factory推論。現在のmutable collection fieldはconstructorの不変条件を利用者が壊せる。
2. physical/node/allocator/compiler/nativeの予算と、全native workの課金。v0.9.9のworkはstdのbyte/text/number primitiveとGCの概算。GCのexternal root、recursive enum/closure/branch/defer/task/channel、native callback途中のsafe pointを境界試験で広く検証する。checkpointを意図的に保持した場合のretentionは仕様として残す。
3. 確定operation ledgerとinput観測の圧縮/spill/回収。ledgerはcheckpointだけでなく外部branch anchorも参照するので、root登録やfloorを設計してから削除する。実行中の無制限streamを約束しない。
4. typed Errorのcode/location/cause統合、fallible comparator/monoidとcapacity/budget/callback失敗のtransaction保証、API diffでの費用/失敗契約の表現。
5. publishのdirectory/staging/rename/delete/short-write部分失敗を構造化し、再試行不能区間と既に反映した内容を明確にする。複数file/streamの一括atomic性を暗黙に追加しない。
6. streamingの絶対offset、incremental CSV/JSON、borrowed slice/view、Unicode/grapheme/regex、wide integer/decimal、Instant/Duration/Calendar。CSVは現在bounded whole-input、matrixはchecked Int64。

## libraryの継続

既存sort/search/sequence、Fenwick/segment、KMP/Z、sparse min、bitset、rollback DSU、graph ordering/Bellman-Ford/SCC/MST/LCA、matrix/LIS/knapsackを再実装しない。lazy segment tree、suffix array/LCP/trie、max/min-cost flow/matching、NTT/convolution/geometry、persistent user-Ord Map/HashMapを費用・overflow・参照解の契約から順に追加する。

アプリ向けにはstructured log、日時/decimal、process lock/schema migration/crash recoveryを持つstore、network/process/DB/UI adapterが残る。adapterはcapability、timeout、idempotency、Secretと不可逆publish境界を固定する。

## 配布と互換性

compiler/language/std/lockを揃え、署名付きSDK/API baselineとsource-free成果物を継続検証する。公式署名鍵とrotation、Rust toolchain pin、source再ビルド、Linux aarch64/macOS/Windowsの実行試験、std upgrade/rollback、runtime-only/embedding ABIを整備する。既存Linux x86_64 SDKを全platform完成と表現しない。

初回の安定版候補は上の優先事項1〜5の仕様と拒否例・部分失敗例・resource retentionを確定する。高度なalgorithmや全adapterを一度に必須にせず、利用可能範囲と未提供範囲を明記する。codex/developで継続し、版branchは増やさない。
