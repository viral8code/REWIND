# REWIND Transactional Runtime Specification v0.1

## 1. 概要

REWINDは、プログラムの実行状態を任意の名前付きCheckpointとして保存し、その状態へ復元・分岐できる実行環境である。

REWINDでは、プログラム状態を単なるメモリ状態として扱わない。

以下をすべて「実行状態」の一部として扱う。

- ローカル変数
- グローバル変数
- Heap
- Stack
- Call Stack
- Program Counter
- VM内部状態
- ファイルシステム変更
- ファイルハンドル状態
- 標準出力
- 標準エラー出力
- 標準入力の読み取り位置

ただし、OSやユーザーなどREWIND VMの外側に存在する実世界そのものを巻き戻すことはできない。

そのためREWINDは、

> 外部副作用を即座に実行せず、仮想I/O層へ蓄積し、明示的な`publish`が行われるまで外界へ反映しない

というTransactional I/Oモデルを採用する。

---

# 2. 基本原則

REWINDの実行状態を次のように定義する。

```text
RuntimeState
├── ComputeState
│	├── ProgramCounter
│	├── Stack
│	├── CallFrames
│	├── Globals
│	└── Heap
│
└── IOState
	├── VirtualFileSystem
	├── FileHandles
	├── StdoutJournal
	├── StderrJournal
	└── StdinCursor
```

Checkpointは、この`RuntimeState`全体を指す。

```text
commit X;
```

は、

```text
Checkpoint["X"] = CurrentRuntimeState;
```

に相当する。

ただし実際には状態全体をコピーせず、Copy-on-Write、Structural Sharing、Journal、Undo Log等によって差分管理する。

---

# 3. External World

REWINDではVM外部を`External World`と呼ぶ。

例えば、

```text
ExternalWorld
├── Host File System
├── Terminal
├── Keyboard Input
├── Process Environment
├── Clock
├── Network
└── Other Processes
```

が該当する。

通常のREWINDコードはExternal Worldを直接操作してはならない。

必ず、

```text
Program
	↓
Virtual I/O Layer
	↓
External World
```

を経由する。

これにより、`publish`以前の副作用を巻き戻し可能にする。

---

# 4. Checkpoint

Checkpointを作成する。

```text
commit base;
```

Checkpointには以下が含まれる。

```text
Checkpoint
{
	name
	parent
	computeRoot
	heapRoot
	fileSystemRoot
	stdoutPosition
	stderrPosition
	stdinCursor
	fileHandleState
}
```

Checkpoint作成によってExternal Worldが変更されることはない。

したがって、

```text
commit base;
```

はDBにおけるCOMMITとは意味が異なる。

REWINDにおける`commit`は、

> 名前付き実行スナップショットの確定

を意味する。

---

# 5. revert

```text
revert base;
```

を実行すると、Current Runtime StateをCheckpoint `base`へ戻す。

例えば、

```text
var a = 10;

commit base;

a = 20;

revert base;

Out.println(a);
```

では、

```text
10
```

となる。

Heap上のオブジェクトについても同様である。

```text
var list = List();

list.add(10);

commit base;

list.add(20);
list.add(30);

revert base;
```

この時点で、

```text
list == [10]
```

となる。

---

# 6. Transactional Standard Output

## 6.1 基本動作

```text
Out.println("Hello");
```

はOSのstdoutへ直接出力しない。

代わりに、

```text
StdoutJournal
```

へ出力イベントを追加する。

例えば、

```text
Out.println("A");

commit X;

Out.println("B");
Out.println("C");
```

では内部状態は、

```text
StdoutJournal

0: "A\n"
1: "B\n"
2: "C\n"
```

となる。

Checkpoint Xは、

```text
stdoutPosition = 1
```

を保持する。

したがって、

```text
revert X;
```

すると論理的なstdoutは、

```text
A
```

だけになる。

`B`および`C`は破棄対象となる。

---

# 7. stdoutはpublishされるまで表示されない

通常モードでは、

```text
Out.println("Hello");
```

を実行しても実端末には表示しない。

例えば、

```text
Out.println("Hello");

commit A;

Out.println("World");

revert A;
```

の後に、

```text
publish;
```

するとExternal stdoutへ出力されるのは、

```text
Hello
```

のみである。

`World`という出力はExternal Worldに一度も存在しなかったものとして扱える。

---

# 8. stderr

stderrもstdoutと同じTransactional Streamとして扱う。

```text
Err.println("error");
```

は、

```text
StderrJournal
```

へ書き込まれる。

Checkpointおよびrevert対象になる。

---

# 9. flush

通常言語における`flush()`はREWINDでは外部出力を意味しない。

```text
Out.flush();
```

はVirtual Output BufferからJournalへの確定のみを意味する。

したがって、

```text
Out.flush();
```

を実行した後でもrevert可能である。

External Worldへの反映は必ず`publish`によってのみ発生する。

---

# 10. Transactional Standard Input

標準入力はstdoutとは性質が異なる。

例えば、

```text
var name = In.readLine();
```

を実行した場合、ユーザーが実際に入力した文字を「入力しなかったこと」にすることはできない。

そこでREWINDは標準入力を、

```text
External Input Journal
```

によって仮想化する。

---

# 11. Input Journal

例えばユーザーが、

```text
Alice
Bob
Charlie
```

と入力した場合、

VM外部のImmutable Input Journalへ、

```text
InputJournal

#0 "Alice\n"
#1 "Bob\n"
#2 "Charlie\n"
```

として保存する。

プログラム状態にはJournalそのものではなく、

```text
stdinCursor
```

を保持する。

例えば、

```text
var a = In.readLine();

commit X;

var b = In.readLine();
```

の時点で、

```text
a = "Alice"
b = "Bob"

stdinCursor = 2
```

になる。

Checkpoint Xでは、

```text
stdinCursor = 1
```

である。

したがって、

```text
revert X;

var c = In.readLine();
```

とすると、新たにユーザー入力を要求せず、

```text
c == "Bob"
```

となる。

---

# 12. 標準入力の重要な例外

標準入力そのものを巻き戻すことはできない。

REWINDが保証するのは、

> プログラムから見える入力ストリーム位置を巻き戻すこと

である。

したがってInput Journalは、通常のRuntimeStateとは異なり、Append-onlyなExternal Observation Logとして扱う。

```text
RuntimeState
	stdinCursor

ExternalInputJournal
	immutable events
```

という構造になる。

これにより、

```text
revert
```

後も、一度読み取った入力を再現できる。

---

# 13. Transactional File System

ファイル操作も直接Host File Systemへ反映しない。

REWIND VMは、

```text
Virtual File System Overlay
```

を保持する。

例えば、

```text
File.write("a.txt", "Hello");
```

を実行しても、

```text
Host:
	a.txt
```

は変更されない。

代わりに、

```text
Overlay:
	a.txt -> "Hello"
```

となる。

---

# 14. Read Your Writes

Transactional File Systemでは、自分自身の変更を即座に読み取れる。

```text
File.write("data.txt", "ABC");

var value = File.readText("data.txt");
```

なら、

```text
value == "ABC"
```

である。

Host File Systemへ反映されている必要はない。

---

# 15. ファイルCheckpoint

例えば、

```text
File.write("data.txt", "A");

commit X;

File.write("data.txt", "B");

commit Y;
```

なら、

```text
X:
	data.txt = "A"

Y:
	data.txt = "B"
```

となる。

その後、

```text
revert X;
```

すれば、

```text
File.readText("data.txt")
```

は、

```text
A
```

を返す。

Host側の`data.txt`は一切変更されていない。

---

# 16. ファイル変更の実装

大規模ファイルをCheckpointごとにコピーしてはならない。

したがってFile StorageもPage単位Copy-on-Writeを基本とする。

例えば、

```text
1 GiB file
```

の1 byteだけを変更した場合、

```text
Old File
	├── Page 0
	├── Page 1
	├── Page 2
	...

New File
	├── Page 0	shared
	├── Page 1	new
	├── Page 2	shared
	...
```

とする。

CheckpointはFile Rootへの参照のみ保持する。

---

# 17. File Operations

以下をTransactional操作として扱う。

```text
File.create()
File.write()
File.append()
File.truncate()
File.delete()
File.move()
File.copy()
Directory.create()
Directory.delete()
Directory.move()
```

例えば、

```text
File.delete("important.txt");

revert base;
```

した場合、

`important.txt`はVirtual File System上で復元される。

External File Systemにはまだ削除操作が送られていないため、Host上では最初から消えていない。

---

# 18. File Handle State

ファイルハンドル自体もRuntimeStateの一部とする。

例えば、

```text
var f = File.open("data.bin");

f.seek(100);

commit A;

f.seek(500);

revert A;
```

すると、

```text
f.position == 100
```

となる。

以下をCheckpoint対象とする。

```text
FileHandle
{
	path
	mode
	position
	buffer
	virtualFileVersion
}
```

---

# 19. Host Fileの読み取り

External File Systemからの読み取りには注意が必要である。

例えば、

```text
var data = File.read("foo.txt");
```

の直後に別プロセスが`foo.txt`を書き換えた場合、revert後に異なる内容が読めては再現性が失われる。

そこでREWINDはExternal File ReadをExternal Observationとして記録する。

---

# 20. File Read Snapshot

Host Fileを初めて開いた時点で、

```text
FileObservation
{
	path
	size
	metadata
	version
	contentHash
}
```

を作成する。

読み取ったBlockはImmutable Read Cacheへ保存する。

```text
Host File
	↓
Read Cache
	↓
Virtual File
```

同じ実行履歴では、一度観測されたBlockについてHostへ再問い合わせしない。

---

# 21. Lazy File Snapshot

巨大ファイルをopen時に全コピーすることは避ける。

そのため、デフォルトではBlock単位Lazy Snapshotを使用する。

```text
FileSnapshot
├── Block 0	captured
├── Block 1	captured
├── Block 2	not loaded
├── Block 3	not loaded
...
```

未取得Blockを初めて読み取る場合、Host Fileのversionを検証する。

Host側ファイルが変更されていた場合、デフォルトでは、

```text
ExternalStateConflict
```

を発生させる。

これにより、一つの論理Snapshot内部で異なる世代のファイル内容が混在することを防ぐ。

---

# 22. Strict Snapshot Mode

必要に応じて、

```text
File.openSnapshot("data.bin");
```

を提供する。

Strict Snapshotではopen時点の内容を完全に固定する。

内部実装は、

- Temporary File
- Memory Mapping
- Content-addressed Storage
- Filesystem Snapshot

等を利用してよい。

Strict Snapshotはメモリまたはストレージ消費量が大きいため、明示指定とする。

---

# 23. branch

Checkpointから別実行系統を作成できる。

```text
commit base;

branch X
{
	File.write("result.txt", "X");
	Out.println("X");
}

revert base;

branch Y
{
	File.write("result.txt", "Y");
	Out.println("Y");
}
```

内部的には、

```text
base
├── X
│	├── result.txt = "X"
│	└── stdout = "X\n"
│
└── Y
	├── result.txt = "Y"
	└── stdout = "Y\n"
```

となる。

External Worldにはいずれも反映されていない。

---

# 24. publish

External Worldへ副作用を確定する唯一の標準操作として、

```text
publish;
```

を定義する。

`publish`はCurrent Runtime StateのExternal Side EffectsをHostへ反映する。

対象には、

```text
File changes
stdout
stderr
```

などが含まれる。

---

# 25. publishは不可逆である

重要な仕様として、

```text
publish;
```

以降のExternal WorldについてREWINDはrollbackを保証しない。

例えば、

```text
Out.println("Hello");

publish;

revert base;
```

を実行しても、既に端末へ表示された、

```text
Hello
```

を消すことはできない。

したがってREWINDでは、

```text
commit
```

と、

```text
publish
```

を明確に区別する。

---

# 26. commitとpublish

```text
commit X;
```

は、

> 内部状態を保存する

操作である。

```text
publish;
```

は、

> External Worldへ副作用を不可逆的に反映する

操作である。

したがって、

```text
commit X;
```

は何度行っても完全に巻き戻し可能である。

一方、

```text
publish;
```

はExternal Transaction Boundaryとなる。

---

# 27. Publish Protocol

`publish`は可能な限りTransactionalに実行する。

基本プロトコルを以下とする。

```text
Phase 1
	Validate

Phase 2
	Prepare

Phase 3
	Apply reversible external changes

Phase 4
	Apply irreversible external changes
```

具体的には、

```text
1. Host File競合確認
2. Temporary File生成
3. File変更準備
4. File replacement
5. stdout/stderr出力
```

の順序を基本とする。

---

# 28. なぜstdoutを最後にするのか

例えば、

```text
File.write("result.txt", result);
Out.println("completed");
```

をpublishするとする。

先に、

```text
completed
```

を表示した後でFile writeに失敗すると、External Worldに矛盾した情報を出してしまう。

したがって不可逆度の高い処理ほど後にする。

基本順序は、

```text
Validation
	↓
Filesystem preparation
	↓
Filesystem commit
	↓
stdout
	↓
stderr
```

とする。

---

# 29. 真のAtomicityに関する制限

OS上では、

```text
複数ファイル変更
+
stdout
```

を完全な単一Atomic Transactionとして処理する一般的な方法は存在しない。

したがってREWINDが保証する強いAtomicityは、

> publish開始前まで

とする。

publish処理そのものについては、ResourceごとのAtomicity Levelを定義する。

---

# 30. Atomicity Level

## Level 0 — Non Transactional

Atomicity保証なし。

## Level 1 — Single Resource Atomic

単一ファイル置換など。

## Level 2 — Filesystem Transaction

同一Transactional Domain内のファイル変更。

可能な環境ではTemporary File + atomic rename等を利用する。

## Level 3 — Virtual Transaction

publish前のREWIND内部状態。

完全なrollbackを保証する。

REWINDの主要保証はLevel 3である。

---

# 31. Publish Conflict

Checkpoint作成以降にHost FileがExternal Processによって変更された場合、

```text
publish;
```

は原則失敗する。

例えば、

```text
commit A;

File.write("data.txt", "X");
```

の途中で別プロセスがHost側の`data.txt`を変更した場合、

```text
publish;
```

は、

```text
ExternalStateConflict:
	data.txt was modified externally
```

を発生させる。

勝手に上書きしてはならない。

---

# 32. Force Publish

明示指定によって競合を無視できる。

```text
publish force;
```

ただしこれは安全性を低下させるため、通常使用は推奨しない。

---

# 33. drop

不要なCheckpointを破棄する。

```text
drop X;
```

Checkpoint Xからのみ参照されている、

- Heap Page
- File Page
- Output Journal
- Metadata

はGC対象となる。

---

# 34. Memory Management

CheckpointのためにRuntimeState全体を複製してはならない。

基本戦略として、

```text
Compute Memory
	Copy-on-Write

Heap
	Object/Page Copy-on-Write

Large Arrays
	Page Copy-on-Write

Files
	Page Copy-on-Write

stdout/stderr
	Append-only Journal

stdin
	Immutable Observation Journal + Cursor
```

を利用する。

---

# 35. Journal Spill

標準出力やFile Deltaが大きくなった場合、RAMに保持し続けない。

一定閾値を超えると、

```text
RAM
	↓
Temporary Storage
```

へspillする。

したがってREWINDの履歴保持コストは、

```text
RAM consumption
```

だけではなく、

```text
RAM + temporary storage
```

として管理できる。

---

# 36. Resource Budget

VMには履歴保持用Budgetを設定できる。

例：

```text
runtime
{
	historyMemory = 512MB;
	historyStorage = 8GB;
}
```

Budgetを超過しそうな場合、

```text
HistoryBudgetExceeded
```

を発生させる。

将来的には、

```text
autoCompact
autoDrop
spill
```

等のPolicyを指定可能とする。

---

# 37. complete example

```text
var a = 10;
var b = 20;

File.write("result.txt", "initial\n");

Out.println("start");

commit base;

a += b;

File.append(
	"result.txt",
	"value=" + a
);

Out.println(a);

// この時点でもHost側には何も起きていない

commit routeA;

a *= 10;

File.append(
	"result.txt",
	"\nrouteA=" + a
);

Out.println(a);

// routeAを捨てる

revert base;

a -= 5;

File.append(
	"result.txt",
	"\nrouteB=" + a
);

Out.println(a);

commit routeB;

// 初めてExternal Worldに反映

publish;
```

Host側から観測可能になるのは最後の`publish`時だけである。

したがって、途中のrouteAで実行された、

```text
a *= 10;
File.append(...);
Out.println(...);
```

はExternal Worldには存在しなかったものとして扱える。

---

# 38. 標準入力を含む例

```text
Out.println("Name?");

var name = In.readLine();

commit answered;

Out.println("Hello " + name);

revert answered;

var name2 = In.readLine();
```

最後の、

```text
In.readLine()
```

では再入力を要求しない。

既にInput Journalへ記録されている値を再生する。

---

# 39. Transaction Boundary

REWINDでは次の境界を定義する。

```text
============================
	REWIND WORLD
============================

完全に巻き戻し可能

Compute State
Virtual Files
Virtual stdout
Virtual stderr
Input Cursor

============================
	publish boundary
============================

原則として巻き戻し不可能

Host Files
Real Terminal
External Systems

============================
	EXTERNAL WORLD
============================
```

---

# 40. Determinism

同一Checkpointから同一入力履歴を使用した場合、

External Worldを新たに参照しない限り、

```text
revert X;
run();
```

は同一結果を生成することを目標とする。

そのため将来的には以下もVirtualizeする。

```text
Clock
Random Number Generator
Environment Variables
Process ID
Filesystem Metadata
Network
External Process
```

---

# 41. Time

例えば、

```text
Time.now();
```

を直接OS Clockへ問い合わせるとreplay結果が変化する。

したがってTimeもObservation Journal化可能とする。

```text
TimeJournal
#0 2026-09-29T10:00:00
#1 2026-09-29T10:00:01
```

revert時には同じ値を再生する。

---

# 42. Random

Random Generatorの内部状態はComputeStateへ含める。

したがって、

```text
commit X;

var a = Random.next();

revert X;

var b = Random.next();
```

では原則、

```text
a == b
```

となる。

---

# 43. v0.1 Scope

REWIND v0.1では以下をTransactional Resourceとして正式サポートする。

```text
Memory
Heap
Stack
Global Variables

Files
Directories

stdin
stdout
stderr

Clock
Random
```

以下はv0.1では未サポートとする。

```text
Network Socket
Database Connection
Child Process
Shared Memory
GPU
Device I/O
OS Native Handle
```

これらはExternal Side Effectとして禁止、または明示的Unsafe APIを要求する。

---

# 44. Unsafe External Operation

どうしてもVirtualizationできない操作については、

```text
unsafe external
{
	...
}
```

を使用する。

このBlock内でExternal Worldへ行われた操作について、

REWINDはrollbackを保証しない。

さらに実行時に現在存在するCheckpointを、

```text
tainted
```

としてマークできる。

---

# 45. Core Invariants

REWIND Runtimeは以下を保証しなければならない。

### Invariant 1

`commit`はExternal Worldを変更しない。

### Invariant 2

`revert X`後のRuntimeStateはCheckpoint Xと論理的に同一である。

### Invariant 3

`publish`以前のFile writeはHost File Systemから観測できない。

### Invariant 4

`publish`以前のstdout/stderrはHost Terminalから観測できない。

### Invariant 5

一度取得されたExternal InputはJournal化され、revert後に再利用できる。

### Invariant 6

Checkpoint間で変更されていないデータは可能な限り共有する。

### Invariant 7

Checkpoint作成時間は状態サイズに比例してはならない。

理想的には、

```text
commit = O(1)
```

またはそれに近い操作とする。

### Invariant 8

External Worldへ不可逆な変更を発生させる標準操作は`publish`のみとする。

---

# 46. REWINDの基本思想

REWINDにおけるプログラム実行は、

```text
Input
	↓
State Transformation
	↓
State Transformation
	↓
State Transformation
```

ではなく、

```text
              ┌─ State B
State A ──────┤
              └─ State C
```

という永続的なState Graphとして扱う。

そしてI/Oも、

```text
Side Effect
```

ではなく、

```text
Proposed External State Change
```

としてRuntime内に保持する。

`publish`だけが、

```text
Proposed State
	↓
External Reality
```

への変換を行う。

したがってREWINDの中心原則を次のように定義する。

> **Nothing becomes real until it is published.**

実行、計算、ファイル変更、出力、分岐。

そのすべてを一度「可能性」として保持し、ユーザーが選択した実行状態だけをExternal Worldへ確定する。

これをREWIND Transactional Runtimeの基本仕様とする。
