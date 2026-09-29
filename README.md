# REWIND

REWIND is a Rust implementation in progress of [the v0.1 transactional runtime specification](docs/REWIND_v0.1.md). It stores named checkpoints of compute state and virtual I/O. Files and terminal output become visible to the host only at `publish;`.

## Run

```sh
cargo run -- examples/route.rw --root /path/to/output-directory
```

The root must already exist. Script file paths are relative to it. The example publishes `result.txt` and prints `start` and `5`; the abandoned route's output and file changes are discarded.

Supported script forms include `var`, assignment, `+=`, `-=`, `*=`, integer and string expressions, `commit`, `revert`, `drop`, `branch NAME { ... }`, `publish`, `publish force`, `runtime { historyMemory = 512MB; historyStorage = 8GB; }`, `Out.println`, `Err.println`, `In.readLine`, file and directory operations, `Time.now`, and `Random.next`. `Out.flush()` and `Err.flush()` only affect the virtual stream. The Rust library also exposes heap objects, stack and call frames, locals, and file handles.

## Current implementation limits

- File handles load host data by block as it is read. `File.openSnapshot` captures the whole file. Whole-file operations such as `File.readText` or `File.write` still materialize the file in memory. Overlay files share 4 KiB pages across checkpoints. File deltas cannot yet spill to temporary storage; exceeding their memory budget returns `HistoryBudgetExceeded`.
- Output journals can spill to temporary storage, and memory/storage budgets cover retained journal segments, overlay file pages, and an estimate of compute values. Host read caches and some metadata are not yet budgeted.
- The CLI is a small interpreter for the documented examples. A complete language VM with function declarations, control flow, and automatic call frames is not implemented. Unsupported external resources are prohibited; the optional `unsafe external` block is not implemented.
- Publishing validates the write set first, stages replacement files, applies files, then writes stdout and stderr. It cannot provide atomicity across multiple files and terminal streams. A failure partway through publication can leave partial host changes.
- Host writes compare full content with the first observed version. Lazy reads compare size and modification time before loading uncached blocks, so an external edit preserving both metadata fields may escape detection. The host root should not be modified concurrently through symlinks or directory renames during a publish.
- Clock replay stores Unix milliseconds. Random uses a deterministic xorshift generator; neither is cryptographic.

See [v0.1 implementation status](docs/v0.1-status.md) for the remaining work. Run `cargo test` to check checkpoint, I/O, replay, conflict and path invariants.
