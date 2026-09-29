# REWIND

REWIND is a programming language implemented in Rust, with named checkpoints, deterministic replay, and virtual I/O. Version 0.3 adds generics, traits, enums, pattern matching, function values and closures, project manifests, environment observations, and development tools. See the [v0.1 specification](docs/REWIND_v0.1.md), [v0.2 design](docs/REWIND_v0.2.md), [v0.3 design](docs/REWIND_v0.3.md), and [v0.3 implementation notes](docs/v0.3-status.md). The next version is described in the [v0.4 proposal](docs/REWIND_v0.4.md).

## Commands

```sh
cargo run -- check examples/v02.rw --root .
cargo run -- run examples/v02.rw --root . --trace
cargo run -- test --root /path/to/project
cargo run -- run examples/v03.rw --root .
cargo run -- fmt examples/v03.rw --check
cargo run -- doc examples/v03.rw --root .
cargo run -- trace examples/v03.rw --root . --trace-json
cargo test
```

`rewind check FILE [FILE ...]` parses, resolves imports, and checks types without executing code. `rewind run [FILE] --root DIR` executes inside an existing project root. With a `rewind.toml` manifest, omitting FILE uses the configured entry; otherwise it loads `DIR/main.rw`. Manifest commands create a missing dependency lockfile and reject changed dependencies until `rewind lock --root DIR` is explicitly requested.

`rewind test [FILE] --filter NAME` runs matching `test fn` functions with separate virtual I/O states. Tests cannot call `publish`. `rewind fmt FILE [--check]` formats or verifies formatting; `rewind doc FILE --root DIR [--output API.md]` generates Markdown public API documentation from signatures and `///` comments. `rewind trace` and the `run --trace` option write checkpoint information to stderr. `--trace-json` selects JSON. Traces are separate from virtual `Err.println` output.

The v0.1 command form `rewind FILE --root DIR` remains available. The same v0.1 scripts also work with `rewind run` without translation.

## Language example

```rewind
fn sum_to(n: Int) -> Int {
    var total = 0;
    for x in 1..(n + 1) {
        total += x;
    }
    return total;
}

let items = List<Int>();
items.add(sum_to(5));
commit base;
items.add(99);
revert base;
Out.println(items);
publish;
```

`let` bindings cannot be reassigned; `var` bindings can. Heap backed List, Map, and struct objects can be changed through a `let` reference. Primitive types are `Bool`, checked 64 bit `Int`, binary64 `Float`, UTF-8 `String`, `Bytes`, and `Unit`. The language provides `List<T>`, `Map<K,V>`, `Option<T>`, and `Result<T,E>`. Map keys can be primitive values or user types implementing `Ord`; inserted user keys are immutable snapshots. Their order is deterministic.

Expressions support arithmetic, comparison, short circuit `&&` and `||`, and `!`. Statements include `if/else`, `while`, `for ... in start..end`, `break`, `continue`, `return`, `match`, `defer`, and `using`. `fn` supports recursion, type parameters, trait bounds, function values, and closures. `pub fn` requires an explicit return annotation. Enum variants and match expressions are checked for exhaustiveness; guards must return Bool. Deferred virtual operations or closures run in reverse registration order at scope exit, return, and error propagation. `using handle = File.open("path");` registers an automatic close.

`import text.format;` loads `text/format.rw` under the source root. `import text.format as fmt;` and `import text.format.{format as format_text};` expose aliases or selected public items. Imports initialize once in dependency order; cycles and paths outside the project root are rejected. A manifest selecting language `0.3` enforces module privacy; projects without that manifest retain v0.2 visibility for compatibility.

## Checkpoints and I/O

`commit NAME;` saves compute state, heap, virtual I/O, call frames, operand stack, and the next bytecode instruction. `revert NAME;` restores that state and continues after the current `revert` statement if its continuation frame still exists. `resume NAME;` restores the saved instruction and call frames, then executes from there. `branch NAME { ... }` records a candidate state and restores the entry state. Checkpoint names are unique across the runtime.

File changes and `Out`/`Err` output remain virtual until `publish;`. Already published effects cannot be undone. `File.readText` and `File.readBytes` return `Result<...,FileError>`; errors expose `code`, `path`, `cause`, and `causes`. `File.writeText` and `File.writeBytes` keep changes virtual. File handles expose `read`, `write`, `seek`, `close`, and `position`. `In.readLine`, `Time.now`, `Env.get`, and `Directory.entries` replay observation journals; `Args.all` uses fixed startup arguments and `Locale.current` uses a fixed locale. `Random.next` uses checkpointed generator state. Pass script arguments after `--`; authorize environment names with `--allow-env NAME` or `--secret-env NAME`, and set locale with `--locale ja-JP`. Traces omit environment values. Network, database, child process, GPU, and device I/O are unavailable.

`runtime { executionSteps = 1000000; }` sets the instruction budget independently of `historyMemory`, `historyStorage`, and `spillThreshold`. The default execution budget is one million instructions.

## Operational boundaries

Publish validates observed host state, stages file replacements, applies file changes, and then emits stdout and stderr. Publication across multiple files and streams is not globally atomic. A partial failure reports `PublishPartiallyApplied` and prevents automatic retry in that runtime. File handles cache observed blocks; `File.openSnapshot` captures the whole file. Details of these v0.1 runtime choices are in [v0.1 status](docs/v0.1-status.md).
