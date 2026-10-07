# Cooperative QR

Run `rewind run main.rw`. A column-pivoted economy QR decomposition yields to
another task, survives a checkpoint/revert, and releases cancelled work.
The example checks Q/R against synchronous QR and compares the permutation.
`numericAsync.qr` returns `QrArrayResult`: its permutation is a native IntArray,
which avoids building an unbounded per-element VM List in the final step.
Public fields are `q`, `r`, `permutation`, and `rank`. Use `numeric.getInt` for
individual permutation entries. `numeric.qr` keeps its original QrResult API.
