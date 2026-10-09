# Cooperative scalar reductions

Run `rewind run main.rw` with an installed SDK. Sum, mean and variance operate
on shared native views, process at most 4096 values per step, and yield between
steps. This example checks explicit task handoff, checkpoint restore and
cancellation. Existing synchronous numeric APIs remain available. The example
does not imply that autodiff backward itself is cooperative.
