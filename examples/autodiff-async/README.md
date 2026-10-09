# Cooperative reverse-mode differentiation

Run `rewind run main.rw`. The example compares the synchronous gradient with a
cooperative backward task, restores the unfinished task and cancels a second
calculation. Expected output: `true`, `true`, `backward done` on separate lines.

`autodiffAsync.backward(move tape, loss)` owns the tape. Keep Node handles for
querying the returned Gradients. Input and gradient numeric pages are shared;
checkpoint roots retain the task state they reference. Existing synchronous
`autodiff.backward(&tape, loss)` remains available. Forward tape construction
retains its synchronous contract.
