# REWIND

REWIND is a Rust implementation of the transactional runtime in [the v0.1 specification](docs/REWIND_v0.1.md), with the consistency decisions recorded in [v0.1 status](docs/v0.1-status.md). It stores named checkpoints of compute state and virtual I/O. Files and terminal output become visible to the host only at `publish;`.

## Run

```sh
cargo run -- examples/route.rw --root /path/to/output-directory
```

The root must already exist. Script file paths are relative to it. The example publishes `result.txt` and prints `start` and `5`; the abandoned route's output and file changes are discarded.

Supported script forms include `var`, assignment, `+=`, `-=`, `*=`, integer and string expressions, `commit`, `revert`, `drop`, `branch NAME { ... }`, `publish`, `publish force`, `runtime { historyMemory = 512MB; historyStorage = 8GB; }`, `Out.println`, `Err.println`, `In.readLine`, file and directory operations, `Time.now`, and `Random.next`. `Out.flush()` and `Err.flush()` only affect the virtual stream. The Rust library also exposes heap objects, stack and call frames, locals, and file handles.

## Boundaries

- File handles load host content by block as it is read. `File.openSnapshot` captures the whole file. Whole-file operations materialize the whole file. A 128-bit content fingerprint is scanned when observing and validating a host version, so lazy storage does not imply constant-time open.
- Output journals, overlay file pages, strict snapshots, and host read blocks share immutable segments that can spill to temporary storage. Configured budgets cover these segments and an estimate of compute values; input/time observation logs and metadata are not included.
- The CLI interprets the documented examples. Function declarations, control flow, and automatic call frames are deferred to the language design. Unsupported external resources are prohibited.
- Publishing validates the write set, stages replacement files, applies files, then writes stdout and stderr. It cannot provide atomicity across multiple files and terminal streams. A partial failure returns `PublishPartiallyApplied` and blocks automatic retry for that runtime instance.
- External file tracking starts at first observation of a path. The host root should not be modified concurrently through symlinks or directory renames during a publish.
- Clock replay stores Unix milliseconds. Random uses a deterministic xorshift generator; neither is cryptographic.

Run `cargo test` to check checkpoint, I/O, replay, conflict and path invariants.
