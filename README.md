# REWIND

REWIND is a Rust programming language with named checkpoints, deterministic replay, and virtual I/O. The transactional runtime is described by the [v0.1 specification](docs/REWIND_v0.1.md). The v0.2 implementation adds typed expressions, control flow, functions, collections, modules, and a bytecode VM that can restore a saved call stack and program counter.

## Commands

```sh
cargo run -- check examples/v02.rw --root .
cargo run -- run examples/v02.rw --root . --trace
cargo run -- test --root /path/to/project
cargo test
```

`rewind check FILE` parses, resolves imports, and checks types without executing code or changing host files. `rewind run FILE --root DIR` executes inside an existing project root. `rewind test [FILE]` runs `test fn` functions with separate virtual I/O states; without a file it loads `DIR/main.rw`. Tests cannot call `publish`. `--trace` writes checkpoint information to stderr, separately from virtual `Err.println` output.

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

`let` bindings cannot be reassigned; `var` bindings can. Heap backed List, Map, and struct objects can be changed through a `let` reference. Primitive types are `Bool`, checked 64 bit `Int`, binary64 `Float`, UTF-8 `String`, `Bytes`, and `Unit`. The language provides `List<T>`, `Map<K,V>`, `Option<T>`, and `Result<T,E>`. Map keys can be Bool, Int, Float, String, or Bytes; their order is deterministic.

Expressions support arithmetic, comparison, short circuit `&&` and `||`, and `!`. Statements include `if/else`, `while`, `for ... in start..end`, `break`, `continue`, `return`, `match`, and `defer`. `fn` supports recursion and block scopes. `pub fn` requires an explicit return annotation. A `defer` expression may call virtual I/O or object methods; it runs on function return or error propagation.

`import text.format;` loads `text/format.rw` under the project root. Imports initialize once in dependency order; cycles and paths outside the root are rejected.

## Checkpoints and I/O

`commit NAME;` saves compute state, heap, virtual I/O, call frames, operand stack, and the next bytecode instruction. `revert NAME;` restores that state and continues after the current `revert` statement if its continuation frame still exists. `resume NAME;` restores the saved instruction and call frames, then executes from there. `branch NAME { ... }` records a candidate state and restores the entry state. Checkpoint names are unique across the runtime.

File changes and `Out`/`Err` output remain virtual until `publish;`. Already published effects cannot be undone. `File.readText` and `File.readBytes` return `Result<...,FileError>`; `File.writeText` and `File.writeBytes` keep their changes virtual. File handles expose `read`, `write`, `seek`, `close`, and `position`. `In.readLine` and `Time.now` use observation journals; `Random.next` uses checkpointed generator state. Environment variables, locale dependent behavior, and file enumeration are not language APIs. Network, database, child process, GPU, and device I/O are also unavailable.

`runtime { executionSteps = 1000000; }` sets the instruction budget independently of `historyMemory`, `historyStorage`, and `spillThreshold`. The default execution budget is one million instructions.

## Operational boundaries

Publish validates observed host state, stages file replacements, applies file changes, and then emits stdout and stderr. Publication across multiple files and streams is not globally atomic. A partial failure reports `PublishPartiallyApplied` and prevents automatic retry in that runtime. File handles cache observed blocks; `File.openSnapshot` captures the whole file. Details of these v0.1 runtime choices are in [v0.1 status](docs/v0.1-status.md).
