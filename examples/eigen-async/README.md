# Cooperative symmetric eigen decomposition

Run:

```sh
rewind run main.rw --steps 20000000 --task-steps 2000000 --native-work 10000000000
```

The example diagonalizes a 65x65 symmetric matrix while another task runs,
restores a checkpoint inside the calculation and cancels a separate task.
Values, vectors and sweep count match synchronous `numeric.eigenSymmetric`.
`numericAsync.eigenSymmetric` returns EigenResult with native FloatArray
`values` / `vectors` and Int `sweeps`; eigenvectors are columns and values are
sorted ascending. Numerical errors are the inner Result, while cancellation
and fatal task budgets are the outer await Result.
