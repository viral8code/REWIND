# Cooperative unary numeric transforms

`rewind run main.rw` runs sigmoid and tanh on reversed, broadcast native arrays.
Another task completes before the longer transform, and a checkpoint restores
its partial result. Cancellation exposes no partial output. The program prints
`true`, `true`, and `unary done`.

Select language 1.9.53 and import `std.numericTransformAsync`. `mapFloat` offers
the existing scalar math operations and `activation` offers the tensor activation
and derivative operations. Each step handles at most 4096 logical values and
hands execution to other tasks between steps. This is a foundation for further
cooperative autodiff; existing `std.autodiff.backward` remains synchronous.
