# Incremental JSON events

`rewind run main.rw` preserves large number literals, decodes UTF-8 / Unicode escapes across byte chunks, drains typed events and restores the queue with a checkpoint. Feed up to 65,536 bytes per call and drain between feeds. Queue limits require retrying the unchanged chunk, with smaller chunks when necessary. Call `finish` to validate EOF. For exact numeric tokens, `nextBigInt` and `nextDecimal` parse before consuming; a wrong event kind or failed conversion stays queued. [bigint.rw](bigint.rw) and [decimal.rw](decimal.rw) demonstrate the two numeric types. `peek(&reader)` inspects the next queued event without consuming it; `next(&mut reader)` advances exactly once. The example checks the opening object with `peek`, then later reads that same queued event after reverting its checkpoint. Event delivery validates syntax incrementally; do not treat a prefix as proof that the entire document is valid.


`nextJson` builds one root value from nested object/array events, rejects duplicate object keys, and accepts at most 64 levels and 65,536 elements per container. Incomplete input, parse errors, and validation errors leave the queue unchanged. `nextValidated` returns the schema-normalized JSON value; application records still use an explicit mapper. JSON number events follow the regular JSON parser's Int/Float representation. Use `nextBigInt` or `nextDecimal` when the exact numeric token must be preserved.


[schema.rw](schema.rw) runs event-to-Json materialization and shared-schema validation. Its expected output is `Ada` then `42`; this example has not been executed.
