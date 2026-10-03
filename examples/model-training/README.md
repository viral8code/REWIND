# Native-array differentiation and model training

```sh
rewind run main.rw --allow-effects fileRead,fileWrite --steps 20000000 --native-work 20000000 --history-memory 256MiB
```

The example fits `y = 2x + 1` using a reverse-mode tape, a two-element matrix parameter and 200 bias-corrected Adam steps. It checks the loss reduction, encodes/decodes the model, publishes `model.rwm`, then loads and compares its native weights. Output: `true`.

A fresh tape is created each step. Values and gradients use native Float64 pages; no VM object is allocated per weight. Tapes and optimizer moments are VM state and can be checkpointed. `publish` makes a saved file physically visible; a later `revert` does not undo that published file. Encoding includes an integrity checksum, not an authenticity signature.

These APIs are synchronous. Work/history/file budgets still apply; the bounded model container is not streaming. The example does not require a Python runtime or another language process.
