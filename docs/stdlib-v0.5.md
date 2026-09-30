# REWIND v0.5 標準 API

以下は実装済み API の署名と効果の要約。空の効果は `effects {}`。すべての可変 VM 状態は checkpoint に含まれる。未公開 I/O は publish まで仮想状態に保持する。

## 値と演算

| 型 | 内容・操作 | 失敗 |
|---|---|---|
| Bool | true/false、!、&&、\|\|、比較 | 型不一致 |
| Int | 符号付き 64 bit、+ - * / %、比較 | overflow、ゼロ除算 |
| Float | binary64、算術と比較 | 不正な演算・型不一致 |
| String | UTF-8、+、比較、iter()->Iterator<String> | 型不一致 |
| Bytes | byte 列、iter()->Iterator<Int> | 型不一致 |
| Unit | 値を返さない関数の結果 | — |

Eq/Ord/Hash/Display はプリミティブと対応する構造に組込み実装を持つ。ユーザー型では impl を定義する。eq は Bool、cmp は Int、hash は Int、display は String を返す。Ord による Map キーは独立した不変 snapshot として保持する。

## コンテナ

| 受信者・構築 | API | 効果・失敗 |
|---|---|---|
| List<T>() | add(T)/push(T)->Unit、set(Int,T)->Unit、get(Int)->T、len()->Int | 純粋、範囲外は実行エラー |
| Map<K,V>() | set(K,V)->Unit、get(K)->Option<V>、remove(K)->Unit、keys()->List<K>、len()->Int | 純粋、K は比較可能 |
| Option<T> | Some(T)、None、match | 純粋、網羅性を検査 |
| Result<T,E> | Ok(T)、Err(E)、match、? | 純粋、? は Err を伝播 |
| Iterator<T> | iter()->Iterator<T>、next()->Option<T> | 純粋、カーソルは snapshot 可能 |
| Frozen<T> | freeze(T)、thaw(Frozen<T>)->T、読取りフィールド/API、iter | 純粋、資源・関数・循環を拒否 |
| Secret<T> | secret(T)、reveal(Secret<T>)->T | 純粋、表示は伏せる |

`for item in value` は iter/next を使用する。Map はキー、String は Unicode scalar、Bytes は 0..255 の Int を返す。標準 Iterator の関連型は Item、next の戻り値は Option<Self::Item>。IntoIterator の iter は Iterator<Self::Item> を返す。List の集約要素を iterator から得る場合は独立した可変コピーになる。

## タスク

| API | 型 | 効果 |
|---|---|---|
| async fn 呼出し | Task<T>（cold） | tasks |
| spawn Task<T> | Task<T> | tasks |
| await Task<T> | Result<T,TaskError> | tasks |
| Task.cancel() | Unit | tasks |
| Task.setPriority(Int) | Unit | tasks |
| Task.ignore()/detach() | Unit | tasks |
| Channel<T>(Int) | Channel<T> | tasks |
| Channel.send(T) | Task<Unit> | tasks |
| Channel.receive() | Task<T> | tasks |
| Channel.close() | Unit | tasks |
| TaskGroup() | TaskGroup | tasks |
| TaskGroup.add(Task<T>) | Unit | tasks |
| TaskGroup.join() | Task<Unit> | tasks |
| TaskGroup.cancel() | Unit | tasks |

チャネル容量は 0..65536。容量 0 は rendezvous。Send は資源を転送できず、可変所有者は move、共有には Frozen を使う。TaskError、Diagnostic、WaitGraph、BudgetKind の構造は [実装規則](v0.5-status.md#2-タスクの終了) を参照。

## 仮想 I/O と観測

| API | 戻り値 | 効果・失敗 |
|---|---|---|
| Out.println(T)、Err.println(T)、flush() | Unit | output。Secret を含む値は伏せる |
| In.readLine() | Option<String> | input、観測 Journal |
| Time.now() | Int | clock、観測 Journal |
| Random.next() | Int | random、保存済み generator 状態 |
| Args.all() | List<String> | args、起動時引数 |
| Env.get(String) | Option<String> | env、明示許可した名前のみ |
| Locale.current() | String | locale、起動時 locale |
| Directory.entries(String) | List<String> | fileRead、順序付き観測 |
| File.readText(String) | Result<String,FileError> | fileRead |
| File.readBytes(String) | Result<Bytes,FileError> | fileRead |
| File.open(String)、openSnapshot(String) | FileHandle | fileRead、失敗は実行エラー |
| File.writeText(String,String)、writeBytes(String,Bytes)、append(String,T) | Unit | fileWrite |
| File.delete(String)、copy(String,String)、move(String,String)、truncate(String,Int) | Unit | fileWrite |
| Directory.create(String)、delete(String)、move(String,String) | Unit | fileWrite |
| FileHandle.read(Int)、readBytes(Int) | String、Bytes | fileRead、失敗は実行エラー |
| FileHandle.write(T)、writeBytes(Bytes) | Unit | fileWrite |
| FileHandle.seek(Int)、close()、position | Unit、Unit、Int | VM 内の資源状態 |

FileError は code:String、path:String、cause:Option<String>、causes:List<String>。using による close と defer は LIFO。publish はアプリケーションのみで、権限検査・Host 状態検証を経て仮想変更を公開する。複数ファイルと出力全体を単一の不可分操作としては公開できない。

## 検査と制御

`assert(Bool)`、`assert_eq(T,T)`、`panic(T)` は純粋な実行検査。commit/revert/resume/drop/branch は VM の保存状態を操作する。runtime ブロックで executionSteps、historyMemory、historyStorage、spillThreshold を指定できる。

これらの説明は [v0.5 実装と運用](v0.5-status.md) と併せて使用する。API の拡張候補は [v0.6 提案](REWIND_v0.6.md) に分けて記載する。
