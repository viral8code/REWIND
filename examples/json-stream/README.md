# Incremental JSON events

`rewind run main.rw` preserves large number literals, decodes UTF-8 / Unicode escapes across byte chunks, drains typed events and restores the queue with a checkpoint. Feed up to 65,536 bytes per call and drain between feeds. Queue limits require retrying the unchanged chunk, with smaller chunks when necessary. Call `finish` to validate EOF. Event delivery validates syntax incrementally; do not treat a prefix as proof that the entire document is valid.
