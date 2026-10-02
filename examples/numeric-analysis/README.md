# 線形代数と統計

REWIND 1.8.1 で `rewind run main.rw`。QR の rank、最小二乗の係数、対称固有値、共分散 / 相関 / 分位点、Bernoulli の endpoint を検証する。出力は `2`、6行の `true`、`4`、2行の `true`。

係数と固有値は誤差を許容して比較する。`std.distributions` は `random` 効果を持つので、明示する project では許可に加える。standalone の既定許可には random が含まれる。純粋な数値計算には追加の外部作用は不要。[契約](../../docs/REWIND_v1.8.1.md)を参照。
