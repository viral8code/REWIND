# 疎行列と FFT

`rewind run main.rw`。出力は `true`、`2`、`true`、`true`。

COO の三つの native 配列から CSR 行列を作り、2×2 の対称正定値行列に対する共役勾配法を task で実行する。実残差を検証し、複素 FFT の round-trip と実数の線形畳み込みも試す。source の List は小さな入力用で、kernel の中間値・行列・結果は native buffer。

FFT と matvec は同期的。共役勾配法は反復の間で VM を譲り、個々の kernel を分割する保証はない。大きな入力では `--native-work` / `--history-memory` を用途に応じて設定する。SPD を前提とし、任意の非対称・特異・不定値行列の solver ではない。
