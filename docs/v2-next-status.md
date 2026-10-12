# v2.1以降：作業状態と再開手順

更新日：2026-10-12。構想から実装へ移行。snapshot/historyに加え12の標準moduleと入れ子Frozenの修正を保存しました。**source実装済み・実行検証はユーザー方針で省略**。版更新・Release公開は行っていません。

## この作業で守る指示

- 作業とcommit/pushは`codex/develop`だけ。今回mainへ統合しない。
- 公開CI/CDを動かさない。commit messageに`[skip ci]`を付ける。
- workflow dispatch、release tag、Release開始message、Release作成をしない。
- 小さなreview可能なまとまりごとにcommit/pushして、未保存の大きな差分を残さない。
- 公開済みv2.0.0の仕様を出発点とし、既存機能を未実装として列挙しない。
- 2026-10-11に構想の実装を依頼された。CP-05/06の一部から着手し、compiler/libraryの変更を小さく保存する。

過去の「公開後mainへ統合」という一般方針は、今回の「codex/developだけ」という指示で上書きされています。将来の実装時も、当時の最新指示を確認してから公開を扱います。

## 継続作業の方針

ユーザーから実行検証を省略して草案の実装を進める指示を受けた。環境復旧を実装の開始条件にしない。[履歴・変更操作library](v2-next-data-tools-implementation.md)へ進め、source実装と検証済みを区別して記録する。

## 継続分の保存履歴

[実装の入口と利用例](v2-next-implementation-index.md)を参照してください。

| commit | 内容 |
|---|---|
| fe1ba3c | bounded Undo/Redo timeline |
| 31f3734 | 型付きMap差分・逆差分・合成・3方向merge |
| 231cdf2 | 親関係・branch HEAD・annotation・祖先query |
| 2c1ac16 | byte parser combinator・期待集合・cut |
| aff915a | 挿入順Map・削除/再挿入・snapshot用case |
| e9ce169 | 入れ子Frozenのwrapperとthaw/iterator materialize |
| 58320b9 | 5 moduleの利用例と草案対応表 |
| c3750d6 | 履歴のpin・祖先保持・削除予定・明示prune |
| fff4017 | 元の変更列の検査とrebase preview・段階root・衝突位置 |
| d09bece | std.textIndex：原文付きbyte/scalar/grapheme位置 |
| b392704 | parser：先読み・原文範囲・条件検査・値依存合成・左右結合 |
| cbeec63 | 複数merge base・最短祖先経路 |
| 4145ec7 | 不要なprune/rebaseのMap再構築を省略 |
| 990ff8a | 利用例・library入口・再開状態を同期 |
| f72308f | 型付きlens・欠損focus・検査付き編集・法則確認 |
| 64f4096 | native共有pageによる永続union-find |
| cb7c13d | 原文を保つ文字置換・旧新位置mapping・逆編集 |
| 3c47bbe | 編集列の合成・失敗step・最終境界・逆順mapping |
| 565dc88 | lens/永続集合/文字編集の利用例と再開状態 |
| 3702363 | JSON schema・default/NULL/unknown policy・型付きdecoder |
| 1fba5f7 | schemaの受入sourceと利用例 |
| 32e98c6 | orderedMapの両端操作・touch |
| 9a2c2a9 | 版付きkey・weight付きLRU・pure Errのcache |
| 9294848 | 明示的な遅延値・成功snapshot・Err保持・独立fork |


これらのtest sourceは実行していません。旧い「まず環境で検証する」という記録は、実行検証省略を許可した今回の方針で更新されています。将来検証するときのcommandとcaseは残します。

## 2026-10-12の再開分

履歴の保持・整理・query、変更列のrebase preview、文字位置型、parserの追加合成を実装した。今回のsource変更に対応するstd testを14件追加した。前日分と合わせた内訳は[対応表](v2-next-implementation-index.md)で確認できる。

rebaseの入力Mapと既存HEADを自動変更しないこと、pin/HEADからの祖先をpruneで保つこと、位置・spanが原文を保持することを契約へ記載した。GCや速度の実測結果はない。機能の名称だけで629構想全体や公開版の完成を宣言しない。

GitHubのrefと差分でsourceの保存を確認した。確認時点のmainは5f5ab8f37c4d21f8746b2d1db077946809e3be42。developのActions一覧の最新実行は2026-10-10の既存commitであり、今回のsource追加による新規実行は見つからなかった。実行検証の代替として扱わない。

利用例はrebase-preview、history-maintenance、text-positions、parser-expressionを追加。expected出力は未実行の期待値。今回もcodex/developのみへ[skip ci]付きでcommit/pushした。

## 2026-10-12の追加継続分

std.lens、std.persistentSet、std.textEditを実装した。lensはShare値とpure callbackを合成し、欠損focusの自動生成を避け、旧値照合とsetterの検査を通す。永続union-findは既存native配列のCOWを使い、統合ごとの全要素copyを追加しない。既存のmutable/rollback union-findは維持した。

文字編集は変更前後のSourceを保持し、削除された位置と曖昧な挿入境界を明示する。最大1,024編集のChainを通して変換でき、失敗したstepを返す。通常のStringと境界表を再構築する編集であり、ropeや増分解析の実装とは区別する。

追加したstd test sourceは12件。typed-lens、persistent-union-find、text-edit-mapping、text-edit-chainに利用例を保存した。compile・実行・Windows・API生成・GC/性能測定は引き続き省略。括弧対応と関数名重複の機械的なsource確認を行ったが、compiler検証の代わりには数えない。

GitHubの差分とrefで保存を確認した。mainは同じ5f5ab8f37c4d21f8746b2d1db077946809e3be42、developのActions最新実行は以前の2026-10-10のまま。今回のcommitはすべて[skip ci]を付け、公開CI/CD・main統合・Release・版更新は行っていない。

## 2026-10-12のschema・cache追加分

DATA-01のJSON部分をstd.schemaへ追加した。入れ子object/array、型と範囲、default、明示NULL、unknown policy、typed pathを扱う。Schemaはdataとして保持でき、Decoderは明示mapperと失敗段階を持つ。CSV/DB/formへの共通adapterやderiveは未実装の範囲として残す。

VCACHE-02/03/04/06/12の基本部分をstd.memoへ実装した。domain/code revision/inputを分けたkeyで複数版を保持し、entry数とcaller指定weightでLRU evictionする。weightを実測memoryと呼ばない。orderedMapへfirst/last/popFirst/popLast/touchを追加し、単一evictionで全keyを列挙する処理を避けた。

VCACHE-07/06の遅延値をstd.lazyへ実装した。peekは計算せず、forceで成功snapshotまたは回復可能なErrを保持する。Sendな可変ownerの結果も独立forkへ渡せる。memo/lazyは通常のVM stateであり、hyper領域や外部I/Oのcacheではない。

追加したstd test sourceは13件。typed-schema、versioned-cache、lazy-snapshotに利用例を保存した。compile・実行・API生成・Windows・GC/性能測定は省略。sourceの括弧対応と同名関数の重複を確認したが、実行検証に数えない。

保存先はcodex/developのみで、すべて[skip ci]付き。確認時点のmainは5f5ab8f37c4d21f8746b2d1db077946809e3be42のまま。Actions最新実行も2026-10-10の既存commitから増えていない。

## 最初の実装単位

[データsnapshotと名前付き履歴](v2-next-snapshot-implementation.md)を追加する。std.snapshot、std.history、埋込みmodule登録、library/CLIの回帰test、利用例を含む。[generic Frozenの共有判定](v2-next-frozen-bounds-implementation.md)も修正する。

CP-05/06全体、動的な実行checkpoint操作、hyper領域は完了扱いにしない。後続のparent graphとMap限定merge/rebaseの実装は継続分の表に記録する。既存のCLI-01以降の計画も維持する。

作業環境のcontainer_creationが失敗し、compileと実行testは未実施。GitHub接続でsourceを調査し、codex/developへ実装と回帰testを保存する。検証できるまで状態を「実装中」とする。

## 今回の保存履歴

| commit | 内容 | 検証 |
|---|---|---|
| 33d3135 | Frozen内部のSend boundを型graphの位置ごとに判定 | 実行待ち |
| 784cbc6 | 呼出側generic boundとFrozen recordの型引数判定 | 実行待ち |
| b76ffaa | snapshot/history、SDK登録、library test、利用例 | 実行待ち |
| 574eac5 | 寿命・revert・source-free artifact/replayの回帰test | 実行待ち |

CLI/artifact/replayの回帰追加を含む最新commitはgit log -- tests/data_history.rsで確認する。保存commitは動作確認完了や版公開の証拠ではない。

## 読む順番

1. [次版ロードマップ](ROADMAP_v2-next.md)：優先順位、外部境界、互換性。
2. [v2.1詳細](REWIND_v2.1-plan.md)：最初の実装対象と受入条件。
3. [v2.2詳細](REWIND_v2.2-plan.md)：GUI/DBの差分と保留条件。
4. [性能計画](v2-next-performance.md)：baselineと最適化の判断。
5. [現行入門書](book/index.html)、[v2.0仕様](REWIND_v2.0.md)、[受入履歴](v2-acceptance.md)：現在の契約を確認する資料。

以前の[2.0ロードマップ](ROADMAP_v2.md)は履歴です。その残件を検査せずに現行残件へ転記しません。

## 保存済みの計画

| commit | 内容 |
|---|---|
| `1254d15` | 全体ロードマップとREADME入口 |
| `840bcc6` | v2.1 CLI/診断/DAP/profile |
| `6189749` | v2.2 layout/component/form-table/DB/helper |
| `2e91344` | 測定条件、GC/共有領域、最適化、長時間運転 |

この状態表自体のcommitは`git log -- docs/v2-next-status.md`で確認できます。計画追加のcommitを実装完了や版公開の証拠にしません。

## v2.1作業一覧

すべて未着手です。詳細な受入はv2.1文書にあります。

| ID | 実装差分 | 先に確認するもの | 完了の証拠 |
|---|---|---|---|
| CLI-01 | root/language context共有 | `src/main.rs`、manifest/standalone | root matrix、artifact互換、移行例 |
| CLI-02 | help/option/JSON統一 | CLI-01、既存subcommand help | 引数/終了値/stdout-stderr確認 |
| DIAG-01 | 所有権・効果の関連位置 | checker、診断schema、LSP位置 | move/borrow/effectとUnicode/redaction |
| DIAG-02 | 継続・begin・external理由 | checkpoint/runtime contract | if内revert、scope/呼出し境界 |
| DIAG-03 | task/cleanupの原因表示 | Failure/TaskError/wait graph | 取消/予算/主失敗＋cleanup失敗 |
| DBG-01 | recorded engine分離 | 既存debug commandとtrace形式 | CLI command同等性、破損trace拒否 |
| DBG-02 | 値ページと制限watch | DBG-01、共有payload/redaction | bounded表示、stale参照、外部操作なし |
| DBG-03 | DAP stdioとclient例 | DBG-01/02、CLI-02 | fake client、source-free/compact、OS |
| PROF-01 | logical work帰属 | 既存profileとnative会計 | on/off一致、inclusive/exclusive |
| PROF-02 | root/共有保持の説明 | PROF-01、GC/ledger | 共有checkpoint、drop、root別説明 |

初期DAPは**記録閲覧**です。live attach、任意関数を呼ぶwatchは未計画の追加範囲です。将来これらを求める場合は別の安全条件と受入を先に書きます。

## v2.2作業一覧

すべて未着手です。IDのGUI-01はv2.2詳細内の作業名であり、性能文書のGUI-01 caseとは区別して参照します。

| ID | 実装差分 | 依存・確認 | 完了または保留の判断 |
|---|---|---|---|
| GUI-01 | 入れ子layout | arrange、edit、composition | overflow/resize/focus/上限 |
| GUI-02 | 再利用component | layout ID、effect/ownership | 2 instance、pure render、入力保持 |
| GUI-03 | field間検証・table編集 | 既存form/table、GUI-01 | row ID、sort/filter、IME、viewport |
| VIS-01 | 限定plot | renderer primitive、numeric view | 費用/互換確認。未成立なら保留可能 |
| DB-01 | 明示record mapping | typed cell/cursor | NULL/列名/精度/bounded batch |
| DB-02 | transaction helper | external/await/affineの最小例 | 終了状態、cancel、unknown、cleanup |
| DB-03 | 限定migration | DB-02、backend lock/DDL | 同時適用、checksum、途中失敗 |
| APP-01 | 入力・一覧・保存の例 | 必要なGUI/DB単位 | 外部境界、失敗表示、shutdown |

DB transaction、form検証、table viewport、協調GUI入力を新規不足として扱わないこと。DB-02のcallback設計が所有権上成立しない場合は、明示handleを優先し、必要なcompiler差分を計画へ戻します。

## 性能段階

| ID | 状態 | 次の成果物 |
|---|---|---|
| PERF-01 | 未着手 | 共通測定formatとbaseline binary/input記録 |
| PERF-02 | 未着手 | 既存scriptの確認と不足case追加 |
| PERF-03 | 未着手 | 固定rootのGC/保持調査、必要なら最小修正 |
| PERF-04 | 未着手 | 実測したhot pathごとの独立改善 |
| PERF-05 | 未着手 | 長時間の資源・cleanup確認 |

最適化目標の数値はbaseline取得後に決めます。「GCがある」という事実だけで漏れなしを宣言しません。checkpoint由来の意図した保持を欠陥扱いしません。

## 追加構想の状態

2026-10-10に34候補を追加し、さらに型設計・メモリ・実行方式の24候補を加えて、合計58候補になりました。すべて**採用判断前**です。既定の実装一覧とは区別し、候補を全部実装する前提にしません。

- [言語表現・型・変更履歴：9候補](v2-next-language.md)
- [通信・サービス・外部資源：8候補](v2-next-services.md)
- [表データ・数値処理・一般library：9候補](v2-next-data.md)
- [配布・品質・継続運用：8候補](v2-next-ecosystem.md)
- [partial・生成・型の抽象化：8候補](v2-next-type-architecture.md)
- [ヒープ外メモリ・arena・native連携：8候補](v2-next-memory.md)
- [実行方式・計算モデル：8候補](v2-next-execution.md)
- [選定表と採用判断template](v2-next-selection.md)

まず既存APIとの差を最小例で確認します。採用した項目だけ、対象版・依存・受入を決めて実装一覧へ追加します。追加構想の保存履歴はgit log -- docs/v2-next-*.mdで確認できます。以上は構想保存時点の記録です。現在の実装状態は冒頭を参照してください。

## 実現難度で絞らない自由構想集

[docs/visionsの入口](visions/README.md)に、48分野・629項目を追加しました。言語、時間軸、GUI、データ、数理、AI、分散、hardware、検証、開発環境、高度algorithm、domain library、interaction、実験的計算、simulationを扱います。

動的commit名から、文字列lookup、opaque handle、型付きsnapshot、複数版の値へ掘り下げた[具体案](v2-next-checkpoint-api.md)も追加しています。

さらに[checkpoint外の変数領域](v2-next-retained-state.md)を追加しました。通常modelを巻き戻し、履歴catalogや試行回数を保持する構想です。beginの扱いは選択肢として記載し、現行仕様は変更していません。

さらに[巻き戻しを使う契約](visions/17-state-contracts.md)として、attempt、Trial<T>、関数のrollback範囲、task群のsavepoint、仮想入力、restart、EntityRef、rollback eventの8構想を追加しました。

これらは**発想を保存する段階**です。既存58候補の採用判断・実装状態・版の到達条件は変更していません。既存機能の発展形も含むため、この629項目を「現行版にない機能」の一覧として転記しません。

自由構想の追加拡張では、257項目から**581項目へ324項目**を加えました。新しい27分野は、時間付き値、永続collection、探索、作用の値、所有権、式、型合成、stream、計算cache、module、backend、snapshot形式、protocol、双方向data、数値contract、確率model、effect handler、並行model、reactive graph、診断、object合成、暗号、DB、iterator、論理rule、自動微分、device計算です。

さらに[12の利用sketch](visions/sketches/README.md)を擬似コードで掘り下げました。設定editor、import、parser、notebook、外部操作、scheduler探索、simulation、native capsule、form、推論、domain言語、live module更新を扱います。すべて未実装APIを組み合わせた例で、現行版の実行例として扱いません。

[12の設計論点](visions/sketches/design-questions.md)には、実行checkpoint/data snapshot、retained owner、begin、branch共有、参照、履歴削除、採用単位、physical操作、cleanup、code版、決定性、便利さと費用を残しました。未決の選択肢です。sketchと設計論点は581項目へ重ねて数えません。

保存履歴はgit log -- docs/visionsで確認できます。分野ごとにcommit/pushしました。構想保存の段階ではruntime/library変更、Release、版更新、main統合、公開CI/CDの起動操作は行っていません。現在の実装状態は冒頭に記載します。

今後さらに思い付いた場合は、対応する分野へ新しいIDを付け、入口の件数を更新します。採用へ進める項目だけ、現行実装との照合と設計・受入を後で行います。

## CLI-01の実装に進む際の手順

1. working tree、branch、origin、最新指示を確認する。未commitのユーザー変更を上書きしない。
2. `src/main.rs`でroot省略と明示rootのcontext選択を追う。`RunOptions.standalone`とmanifest検証、source/artifact入口を確認する。
3. 現行構文とstdを使う最小sourceで、manifestなしのroot省略/明示の違いをローカルで再現する。
4. manifestあり、不正manifest、異なるcwd、root外sourceを含むmatrixを作る。旧languageの移行例を実際に確かめる。
5. resolverの入力/出力とerrorを決め、sourceを扱う入口で共用する最小差分を実装する。
6. 対象のローカル確認と必要な回帰確認を行い、結果を短い記録に残す。公開CIは起動しない。
7. 実装・文書・検証をcommit/pushし、この表を更新する。版番号や公開は別判断。

調査中に現行実装が変わっていたら、計画の前提を修正します。すでに解決している差分を再実装しません。

## 状態を更新する規則

状態は「未着手」「設計確認」「実装中」「検証済」「保留」を使います。「検証済」には実装commit、実行command、確認環境、結果、未実施のbackend/OSを添えます。設計文書だけで検証済にしません。

保留は理由と解除条件を必須にします。未完了を隠すためにpatch版を積み上げず、作業単位を完了させてから版全体の達成条件に照らします。性能段階では「差なし」も結果として残します。

各まとまりで以下を記録します。

~~~text
作業ID / 状態:
実装commit:
確認command・入力:
環境 / 結果:
互換性・外部境界への影響:
未確認・保留理由:
次に進める作業ID:
~~~

計画を変更するときは、到達条件・依存・受入・この状態表を揃えます。runtime未実装のAPIを現行入門書やSDK referenceに掲載しません。


## 2026-10-12のCSV schema接続

std.schemaへ`csvRow`を追加した。object schemaをheader名で参照し、既存`std.csvStream`が返す各rowを同じrequired/default/unknown policyで検証できる。Text/Anyはraw textのまま、typed cellは全体をJSON値としてparseする。nullable cellのNULL sentinelは呼出し側が正確な文字列で指定する。row幅・重複header・scalar長・構造visitを制限する。

nested node tableを合成するときに、object field名からchild node indexへのMapも一緒にrelocateする。CSV headerの検索でschema field一覧を毎回scanしない。追加したsourceは[b415648](https://github.com/viral8code/REWIND/commit/b415648e893361523e0edf15d28c48922ddd0f76)。利用例は[typed-csv-row.rw](../examples/typed-csv-row.rw)。

今の範囲はJSON/CSV rowまで。DB/form向け共通adapter、Decimal/date ruleは残る。sourceは保存済みだが、compiler/実行/API生成/性能計測はしていない。test sourceは今回追加しておらず、公開CI/CD、main統合、Release、版更新もしていない。


## 2026-10-12のCSV stream連携

既存std.csvStreamに`nextTyped`を追加し、既に保存した`schema.csvRow`へ一行ずつ接続した。readerのpeek後にschema変換し、成功した場合だけadvanceする。Validation failureなら同じ行を残し、schemaを直して再試行するかraw `next`で明示的に捨てられる。stream failureとschema failureは`TypedRowError::Stream/Validation`で区別する。

変更は[db5b39d](https://github.com/viral8code/REWIND/commit/db5b39d8f50de71b690c3666988e3f7f99a1c20a)。[csv-stream example](../examples/csv-stream/README.md)を型変換・失敗後の再試行を見せる内容に更新した。新たなtest sourceは追加していない。環境起動失敗によりcompile/runtime確認は未実施。

DATA-04全体は未完了。実装したのは既存CSV queueを使う1-row consumerであり、JSON record adapter、DB/HTTP cursor共通化、batch collection APIは残る。queue管理・chunk再送・cancelは既存reader契約を使い、std.schemaは入力streamや外部資源を所有しない。


## 2026-10-12のDB column mapping helper

v2.2のDB-01に`std.db.ColumnMap`、`indexColumns`、`columnNumber`を追加した。cursor metadataの列名を最大256件まで一度検査し、重複名を拒否してimmutable Mapへ保持する。同じbatch内の各rowでname lookupを繰り返せる。record生成とInt/Text等の選択は既存typed accessorを使って呼出側に残した。

[SQLite](../examples/database/README.md)と[PostgreSQL](../examples/postgres/README.md)の両exampleで、backendを越えて同じColumnMap APIから列位置を引き、typed accessorを使うuser mapperがScore / StatusRowを構築する。既存の1件ずつ読むcursorを維持し、全rowを収集しない。新しいtest sourceは追加していない。実行環境のcontainer起動失敗によりcompile/runtime確認は未実施。SQLite/PostgreSQLの両方での実行受入、NULL/duplicate/missing、cleanup確認はDB-01の残り。

## JSON event streamの再開分

2026-10-12に`std.jsonStream.peek(&Reader)`と`nextBigInt(&mut Reader)`を追加しました。先頭のqueued eventを検査してもreaderをadvanceせず、整数tokenは正確な10進BigIntへのparse成功後にだけ消費します。型違い・変換失敗ではeventをqueueに保ち、再解釈できます。API manifest、library一覧、DATA-04の実装状況、2つの利用例も更新しました.

これはJSON streamからschema/recordを組み立てるadapterそのものではありません。DATA-04にはrecord materializationとDB cursor/HTTP sourceとの共通batch APIが残っています。

source・例のcompile/runtime/API再生成とWindows確認は未実施です。commit `1f2052d` はcodex/developへ `[skip ci]`付きで保存しました。公開CI/CD、main統合、releaseは行っていません。

## JSON event streamの再開分

2026-10-12に`std.jsonStream.peek(&Reader)`を追加しました。先頭のqueued eventを検査してもreaderをadvanceせず、既存の`next(&mut Reader)`だけがeventを消費します。利用例はpeek後も同じObjectStartを読めることを示します。API manifest、library一覧、DATA-04の実装状況も更新しました。

これはJSON streamからschema/recordを組み立てるadapterそのものではありません。DATA-04にはrecord materializationとDB cursor/HTTP sourceとの共通batch APIが残っています。

source・例のcompile/runtime/API再生成とWindows確認は未実施です。commit `1f2052d` はcodex/developへ `[skip ci]`付きで保存しました。公開CI/CD、main統合、releaseは行っていません。
