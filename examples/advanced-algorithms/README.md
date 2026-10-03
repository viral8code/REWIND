# 標準アルゴリズム

`rewind run main.rw` で遅延評価セグメント木、Trie、suffix 検索、整数幾何処理を確認する。出力は `28`, `2`, `1`, `1`。木の更新を checkpoint から戻し、native suffix array と LCP は shared IntArray に保持する。
