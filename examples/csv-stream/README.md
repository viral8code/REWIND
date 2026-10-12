# Incremental CSV with schema validation

`rewind run main.rw` feeds one byte at a time across UTF-8, doubled quotes and CRLF. It reads the header, tries a schema that expects a boolean cell to be an integer, observes `TypedRowError::Validation`, then retries the same queued row with the correct schema. The row advances only after successful normalization. After publishing, `revert rows` restores the reader so the raw header and row can be read again.

For larger inputs, feed up to 65,536 bytes at a time and drain rows after each successful feed. A failed feed leaves the reader unchanged; after a queue limit, drain and retry the same chunk in smaller pieces. A typed validation error also leaves its row queued; use raw `next` only when deliberately discarding that row. `finish` validates EOF; `cancel` discards pending data and closes the reader.

Execution is unverified in this environment.
