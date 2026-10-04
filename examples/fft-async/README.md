# Cooperative FFT

```sh
rewind run main.rw --native-work 100000000 --steps 20000000 --task-steps 2000000
```

The example transforms a constant vector while another Task completes, restores
an unfinished transform from a checkpoint, then applies the inverse transform.
It prints `fft done`. It performs pure VM computation and needs no external
permission, listener, database or input file.
