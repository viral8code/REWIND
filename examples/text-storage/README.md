# Immutable text and retained copies

Run `rewind run main.rw`. The output is `text restored`.

The example keeps text in a value capture, a List, a Map and a checkpoint,
changes the active values, then restores the checkpoint. Long VM text values
share immutable storage; changing a value preserves the retained contents.
This does not make history budgets an exact measurement of physical memory.
