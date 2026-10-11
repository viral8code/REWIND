# データsnapshotと名前付き履歴：最初の実装

更新日：2026-10-11。状態：**実装中・実行検証待ち**。

[構想 CP-05/06](v2-next-checkpoint-api.md)から、既存Frozen<T>を使うdata snapshotと、文字列の名前を付ける履歴registryを実装する。現在の公開版のreferenceではなく、codex/developの実装記録。

## この単位の範囲

- std.snapshot：Snapshot<T>、capture、read、fork。
- std.history：History<T>、HistoryError、create、length、contains、save、get、names、remove、clear。
- compiler内の標準module登録。standalone importとSDK生成の両方で同じsourceを使用する。
- generic Frozenの共有判定と、Frozenをfieldに持つrecordの型引数判定。
- library内の回帰case、CLI/artifact/replayの回帰case、利用例。

実行checkpointのcreate/lookup/restore、opaqueなCheckpointId、parent graph、merge、hyper領域は別の実装単位。CP-05/06全体を完了扱いにしない。

## 値の契約

Snapshot<T>はprivateなFrozen<T>を持つimmutable record。T:Sendを要求し、captureは既存freeze、forkは既存thawを利用する。通常の代入でsnapshotを共有できる。

capture(&value)は呼出し時のdataを保存する。元のmutable valueを後から編集しても保存値は変わらない。read(saved)はFrozen<T>を返し、fork(saved)は独立した値を返す。関数が終了した後でも、返されたSnapshot<T>を利用できる。

保存するのは選択した値であり、program counter、call frame、全VMの変数一覧ではない。外部接続や任意関数の保存は、既存freezeの型・実行時制約に従う。Sendだけで全native resourceが静的に排除されるわけではなく、freezeの実行時検査も残る。

## API

すべての関数はeffects {}。T:Sendを要求する。

| module | signature |
|---|---|
| snapshot | `capture<T:Send>(value:&T)->Snapshot<T>` |
| snapshot | `read<T:Send>(saved:Snapshot<T>)->Frozen<T>` |
| snapshot | `fork<T:Send>(saved:Snapshot<T>)->T` |
| history | `create<T:Send>(capacity:Int)->Result<History<T>,HistoryError>` |
| history | `length<T:Send>(history:&History<T>)->Int` |
| history | `contains<T:Send>(history:&History<T>,name:String)->Bool` |
| history | `save<T:Send>(history:&mut History<T>,name:String,value:&T)->Result<Unit,HistoryError>` |
| history | `get<T:Send>(history:&History<T>,name:String)->Option<Snapshot<T>>` |
| history | `names<T:Send>(history:&History<T>)->List<String>` |
| history | `remove<T:Send>(history:&mut History<T>,name:String)->Option<Snapshot<T>>` |
| history | `clear<T:Send>(history:&mut History<T>)->Unit` |

## 名前付きregistry

create<T>(capacity)は1〜65,536の登録上限を受け取る。名前は1〜128 Unicode scalar。文字列を組み立てて、save、get、removeへ渡せる。

saveは名前の長さ、重複、容量をこの順番で検査し、その後にcaptureして登録する。失敗はHistoryError::InvalidCapacity、InvalidName、DuplicateName、Capacity。欠落した名前のget/removeはNone。

同じ名前の上書きは行わない。removeして同じ名前を再利用しても、既に取得したSnapshot<T>は旧dataを指す。名前はHistory<T>内のlookup keyで、実行checkpoint labelや大域identityではない。

namesは独立したList<String>を返す。順序はString Mapのkey順。返したListを編集してもregistryは変化しない。

## 所有権と巻き戻し

History<T>は通常のmutable model。操作には共有borrowまたはmutable borrowを使う。registryに対するsave/remove/clearも通常のcheckpointで戻る。

removeは登録を消し、そのsnapshotを返す。clearはregistryの全登録を解除する。他の変数、独立snapshot、実行checkpointが保持するdataは、それぞれのownerがなくなるまで残る。

beginの仕様は変更しない。hyperglobalな履歴ではなく、revert begin後も自動的に生き残るregistryではない。

## 費用

captureは既存freezeのgraph検査、materialize、共有page保持の費用を持つ。常にO(1)とは約束しない。循環、深さ、native work、allocation等の制約もそのまま適用する。

forkは既存thawの費用を持ち、大きなcollectionを走査・構築する場合がある。今回のwrapper追加を、高速なpersistent forkの完成として数えない。

capacityは登録件数の上限。memoryの上限ではない。保存dataのサイズと、registry以外で保持したsnapshotもVMの既存会計・GCの対象となる。

typedな事前検査失敗はregistryを変えない。fatalな実行予算不足まで一括atomicに回復する保証は追加しない。

## 利用例

[examples/data-history.rw](../examples/data-history.rw)を用意した。新しい実行ファイルでの想定操作：

~~~sh
rewind run examples/data-history.rw
~~~

想定出力：

~~~text
10
99
7
10
~~~

最後の10はregistryをclearした後に、取得済みsnapshotを読み出した値。例の実行結果はまだ未確認。

## 追加した回帰test

- capture後の元List変更と、二つのforkの相互独立性。
- mutableな入れ子struct/collectionの独立性。
- 同じ名前の重複拒否、容量、空名、Unicodeの長さ上限。
- 名前の削除・再利用・clear後も、取得済みsnapshotの値が変わらない。
- namesの返値を編集してもregistryが変わらない。
- registryが通常modelとしてrevertされる。
- captureした関数の終了後にもsnapshotを利用できる。
- sourceとstandard-library cacheを削除したartifact実行、compact record/replay。
- Frozen viewの変更、private field・constructorへのアクセス、FileHandle captureの拒否。

libraries/std/tests.rwに3 test、tests/data_history.rsに4 test、tests/generic_frozen_bounds.rsに型判定の3 testを追加した。いずれも実行未確認。

再開時の実行command：

~~~sh
cargo test --test generic_frozen_bounds
cargo test --test data_history
cargo test --test language_v05 --test language_v06
cargo run -- test --root libraries/std
cargo run -- run examples/data-history.rw
cargo fmt --check
python3 scripts/check-standard-library-lock.py
~~~

## 検証状態と次の作業

2026-10-11の作業環境はcontainer_creationで失敗した。端末・filesystemの実行toolが提供されず、GitHub接続でsourceを確認して実装を保存した。上記commandの実行、API snapshot生成、性能・GC測定は未実施。確認完了まで検証済みに変更しない。

保存後にGitHub上の差分とbranch refを照合した。codex/developには今回の変更があり、mainは5f5ab8fのまま。4件の実装commitを対象にActions一覧を確認し、起動されたworkflow runはなかった。これらはsource保存の確認であり、compileや動作確認ではない。

最初に型検査・所有権検査を実行し、generic/borrowに起因する問題があれば修正する。その後、source-free/replayと標準library全体を確認する。read/forkの費用は現行基盤の測定結果に従って記録する。

型判定の変更は[別の実装記録](v2-next-frozen-bounds-implementation.md)を参照する。新しいsource moduleのAPI snapshotはSDK生成の既存処理へ接続しているが、生成結果は未確認。

次の候補は、独立data rootを保つforkの費用改善、version metadataとparent関係、最後に実行checkpointの動的lookupとcontinuationの制約。名前付きregistryを実行checkpoint APIの代替実装として扱わない。

保存先はcodex/develop。公開CI/CD、main統合、Release、版更新は行わない。
