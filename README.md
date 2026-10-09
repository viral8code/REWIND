# REWIND 1.9.58

REWIND は `commit` / `revert` / `publish` で VM 内の状態と出力を扱う言語です。v1.9.58 では自動微分の逆伝播を小さく区切り、他のタスクに処理を渡せるようにします。[変更点](docs/REWIND_v1.9.58.md)を参照してください。

`rewind run main.rw` / `rewindc main.rw`、Checkpoint と増分 publish、署名付き Linux / Windows x64 SDK を提供します。1.5 では即時外部操作の明示領域と、巻き戻し後の結果再利用を追加しました。[使い方](docs/getting-started.md)、[言語リファレンス](docs/language-reference.md)、[GUI](docs/gui.md)、[1.5 の変更点](docs/REWIND_v1.5.md)、[ライブラリ](libraries/README.md)を参照してください。開発 branch は `codex/develop` です。各版の検証後、`main` に統合して Release を公開します。

今後の設計は [v2.0 までの実装計画](docs/ROADMAP_v2.md) と [詳細設計案](docs/v2-design.md) を参照してください。後続機能は計画段階です。

## SDKを試す

[GitHub Releases](https://github.com/viral8code/REWIND/releases/latest)で Linux x86_64 / Windows x64 SDK を配布します。[導入](docs/getting-started.md)を参照してください。

v0.9 adds multiline REPL sessions with verified transcripts, API contracts for implementations and dependencies, production-only installs, Share generic records, ordered property shrinkers, and signed inspection timelines. Select `language = "0.9"`; see [implementation and limits](docs/v0.9-status.md) and the [v0.9.1 application proposal](docs/REWIND_v0.9.1.md).

v0.8 adds signed development dependencies, a transactional virtual-I/O REPL, one-generation read-only trace compatibility, and non-executable inspection exports. Select `language = "0.8"`; see [commands and limits](docs/v0.8-status.md) and the [v0.9 proposal](docs/REWIND_v0.9.md).

v0.7 adds immutable records, checked pure constants, public API snapshots/diffs, isolated documentation examples, typed property generators, and source-free timelines. Select `language = "0.7"`; see [implementation and limits](docs/v0.7-status.md) and the [v0.8 proposal](docs/REWIND_v0.8.md).

# REWIND

v0.6 adds public closure Send/Share contracts, temporary synchronous borrow captures, specialized generic effects, tuple patterns, type aliases, default methods, logical task timeout/select, reproducible integer property tests, authenticated persistent caches, signed mirror resolution with update preview/apply, LSP editing support, source-free reverse debugging, and external Secret observations with audits. Select `language = "0.6"`; see [APIs, limits, and compatibility](docs/v0.6-status.md) and the [v0.7 draft](docs/REWIND_v0.7.md).

REWIND is a programming language implemented in Rust, with named checkpoints, deterministic replay, and virtual I/O. Version 0.5 adds Frozen snapshots, move and lexical borrow checks, transferable closures, typed task errors, function effects, verified source-free artifacts, generic impls, associated types, iterators, transitive signed dependencies, LSP, and a recorded interactive debugger. See the [v0.1 specification](docs/REWIND_v0.1.md), [v0.2 design](docs/REWIND_v0.2.md), [v0.3 design](docs/REWIND_v0.3.md), [v0.4 implementation notes](docs/v0.4-status.md), [v0.5 implementation notes](docs/v0.5-status.md), and [standard API](docs/stdlib-v0.5.md). The v0.6 design is described in the [v0.6 specification](docs/REWIND_v0.6.md).

## Commands

```sh
cargo run -- check examples/v02.rw --root .
cargo run -- run examples/v02.rw --root . --trace
cargo run -- test --root /path/to/project
cargo run -- run examples/v03.rw --root .
cargo run -- fmt examples/v03.rw --check
cargo run -- doc examples/v03.rw --root .
cargo run -- trace examples/v03.rw --root . --trace-json
cargo run -- run examples/v04.rw --root . --record session.json
cargo run -- replay session.json --root .
cargo run -- debug session.json
cargo run -- profile examples/v04.rw --root .
cargo run -- test --root /path/to/project --explore 100
cargo run -- update --root /path/to/v04-project
cargo run -- build --root /path/to/v04-project --output rewind.build.json
cargo run -- update --root examples/v05
cargo run -- run --root examples/v05
cargo run -- build --root examples/v05 --output program.json
cargo run -- run-artifact program.json --root examples/v05 --allow-effects output,tasks
cargo run -- lsp --root examples/v05
cargo run -- debug-session session.json --root examples/v05
cargo run -- migrate --root /path/to/project
cargo test
```

`rewind check FILE [FILE ...]` parses, resolves imports, and checks types without executing code. `rewind run [FILE] --root DIR` executes inside an existing project root. With a `rewind.toml` manifest, omitting FILE uses the configured entry; otherwise it loads `DIR/main.rw`. v0.2/v0.3 manifests create a missing dependency lockfile and reject changed dependencies until `rewind lock --root DIR` is explicitly requested. A v0.4 manifest requires an explicit initial `rewind update` and verifies package signatures, version requirements, capabilities, and the exact lockfile on subsequent commands.

`rewind test [FILE] --filter NAME` runs matching `test fn` functions with separate virtual I/O states. Tests cannot call `publish`. `rewind fmt FILE [--check]` formats or verifies formatting; `rewind doc FILE --root DIR [--output API.md]` generates Markdown public API documentation from signatures and `///` comments. `rewind trace` and the `run --trace` option write checkpoint information to stderr. `--trace-json` selects JSON. Traces are separate from virtual `Err.println` output.

The v0.1 command form `rewind FILE --root DIR` remains available. The same v0.1 scripts also work with `rewind run` without translation.

`async fn` calls create cold `Task<T>` values; `spawn`, `await`, or `TaskGroup.add` starts them. In v0.5, `await` returns `Result<T,TaskError>`; earlier modes retain `Result<T,String>`. `Channel<T>(capacity)` supports awaited send/receive, including rendezvous at capacity zero. Checkpoints restore all task states and channel queues together. v0.5 transfers independent owners, including closures whose captures are Send. File handles and cyclic graphs cannot cross task boundaries. From v1.6.1, HTTP resources can move across tasks but cannot be shared or frozen; their Task results transfer once. Before v1.6, `publish` requires the application task and completed or cancelled children. From v1.6, the application task may publish current virtual I/O while children remain pending.

`run --record FILE` records observations and instruction order, including failed v0.5 executions. `replay FILE` checks the program and observations without changing host files. `debug TRACE.json` inspects saved states; `debug-session TRACE` offers step/continue/checkpoint/state/tasks/files. `profile` reports logical storage and task instruction counts to stderr. `test --explore N --record FILE` can save a failing schedule for replay. v0.5 `build` produces a verified artifact for `run-artifact`; older modes retain bytecode templates. Details and limits are in the [v0.5 implementation notes](docs/v0.5-status.md).

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

File changes and `Out`/`Err` output remain virtual until `publish;`. Already published effects cannot be undone. `File.readText` and `File.readBytes` return `Result<...,FileError>`; errors expose `code`, `path`, `cause`, and `causes`. `File.writeText` and `File.writeBytes` keep changes virtual. File handles expose `read`, `write`, `seek`, `close`, and `position`. `In.readLine`, `Time.now`, `Env.get`, and `Directory.entries` replay observation journals; `Args.all` uses fixed startup arguments and `Locale.current` uses a fixed locale. `Random.next` uses checkpointed generator state. Pass script arguments after `--`; authorize environment names with `--allow-env NAME` or `--secret-env NAME`, and set locale with `--locale ja-JP`. Traces omit environment values. HTTP/HTTPS use recorded external operations. Database, child process, GPU, and device I/O are not yet available.

`runtime { executionSteps = 1000000; }` sets the instruction budget independently of `historyMemory`, `historyStorage`, and `spillThreshold`. The default execution budget is one million instructions.

## Operational boundaries

Publish validates observed host state, stages file replacements, applies file changes, and then emits stdout and stderr. Publication across multiple files and streams is not globally atomic. A partial failure reports `PublishPartiallyApplied` and prevents automatic retry in that runtime. File handles cache observed blocks; `File.openSnapshot` captures the whole file. Details of these v0.1 runtime choices are in [v0.1 status](docs/v0.1-status.md).

## v0.9.1

JSON・設定/引数・application entry/exit status・asset 付き production 配布と最初の標準ライブラリを実装しました。[実装状況](docs/v0.9.1-status.md)、[ライブラリ](libraries/README.md)、[オフライン CLI 例](examples/v091/README.md)、[SDK と library の v0.9.2 草案](docs/REWIND_v0.9.2.md) を参照してください。

## v0.9.2

署名付き SDK の組立て・検証・std の project 導入と、text/bytes/number/bits/result/map を追加しました。[実装状況](docs/v0.9.2-status.md)、[SDK仕様](docs/REWIND_v0.9.2.md)、[利用例](examples/v092/README.md)を参照してください。主要な処理・データ構造・アルゴリズムと性能改善の次段階を [v0.9.3草案](docs/REWIND_v0.9.3.md)にまとめています。

## v0.9.3

List/heapのpersistent storageと、基本アルゴリズム・データ構造・区間/グラフ処理・byte scannerを追加しました。[実装状況](docs/v0.9.3-status.md)、[最短距離CLI](examples/v093/README.md)、[v0.9.4計画](docs/REWIND_v0.9.4.md)を参照してください。今後の作業branchは`codex/develop`です。

PostgreSQL の opaque credential alias、verified TLS、逐次 cursor は [v1.7.1](docs/REWIND_v1.7.1.md) と [実行例](examples/postgres/README.md) を参照。

`std.training` provides named gradient extraction, mean-square loss, sample-weighted batch-mean merging and global-norm clipping. See [the nonlinear training example](examples/nonlinear-training/README.md).
