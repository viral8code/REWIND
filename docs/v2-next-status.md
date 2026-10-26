# v2.1以降：作業状態と再開手順

更新日：2026-10-12。構想から実装へ移行。snapshot/historyに加え12の標準moduleと入れ子Frozenの修正を保存しました。**source実装済み・実行検証はユーザー方針で省略**。版更新・Release公開は行っていません。

## 現状（2026-10-19）

この文書の以下の表は、v2.1〜v2.2を計画していた当時の記録で、**現在の状態とは一致しません**。現在の実装は REWIND 3.6.0（`codex/develop`）です。

- v2.1の作業（CLI-01/02、DIAG-01〜03、DBG-01〜03、PROF-01/02）は、v3.1で出荷済みです（[v3.1](REWIND_v3.1.md)）。
- DB transaction・migration・PostgreSQLの実backend確認は、v3.1〜v3.3で行いました。LANG・NET・DATA・QUAL・MEMの一部の構想は、v3.4〜v3.6で採用・実装されました。
- 言語の現在の仕様は[言語リファレンス](language-reference.md)、入門は[入門書](book/index.html)にあります。
- 2026-10-20: 標準moduleに`std.dna`（2bit packing、k 1..32、k-mer・逆相補・canonical・roll・GC数・Hamming距離）と`std.fastx`（FASTA/FASTQの解析と整形、phred）を追加しました。版は上げていません。
- 2026-10-20: 標準moduleに`std.web`（contentType、safeJoin、etag、notModified、parseRange、securityHeaders、session cookie、parseForm、formValue、redirectLocation の純粋helper）を追加しました。版は上げていません。
- 2026-10-20: `numeric.matmul`を内部で行単位に並列化しました（`std::thread::scope`、スレッド数は最大8）。結果はビット単位で直列版と同一で、言語の意味は変えていません。m*n*k が 2^21 以上で m が 2 以上のときだけ並列になり、公開APIは変えていません。版は上げていません。
- 2026-10-20: ローカル変数の文字列 `+=` を、ヒープ上の値を移動して追記するようにしました。追記の費用は追加分に比例し、以前のように文字列全体を複製しなくなりました。言語の意味、`commit`/`revert` の結果、実行予算とメモリの計上は変えていません。フィールドへの `+=`（`self.buf += ...`）は対象外です。版は上げていません。
- 性能段階（PERF-02〜05）の測定結果は[v2-next-perf-results.md](v2-next-perf-results.md)にあります。
- 3.6.0の後に `codex/develop` へ入った言語の改善（版は上げていません）: 借用した`List`/`Map`を`for`で回せるようにした、`?`のエラー型の不一致（組み込みの型どうし）と`Result`を返さない関数での`?`を実行前に拒否、関数の型制約に存在しない名前を拒否、型を省ける短いクロージャ（`|n| n * 2`。型が決まる場所、つまり関数の引数、`Iterator`・`Result` のメソッド、`return`、代入、型注釈つきの `let` でだけ。`language` 3.0.0 以上）、カンマ区切りのすべての箇所での末尾のカンマ、C# 方式の raw 文字列（`"""` で開閉し、エスケープも補間も効かない）、`const`と`Map.keys()`の`for`の修正。
- Windowsのネイティブ GUI は、ローカルの実機で実際のウィンドウを開き、マウス・キーボード入力、Undo、保存、リサイズ後の再配置を確かめました（[docs/gui.md](gui.md)、`scripts/gui-smoke-windows.ps1`）。Linux/X11（WSLg の XWayland）でも、同じ確認を行い、`resize` イベントが止まらなくなる不具合を直しました（`scripts/gui-smoke-linux.sh`）。IME の変換中表示と、Wayland のネイティブ実装の確認は残っています。
- 残っているもの: GUIの自然寸法測定・IME再描画、PERF-01の共通測定形式、Linux・GUI・HTTP・PostgreSQLの測定、言語の使い勝手の改善のうち、`unwrap`/`expect`と入力前の自動`publish`は、「失敗を無視しない」「`publish`するまで外へ出ない」という言語の方針と衝突するため採用していません（[v3.0の未実装一覧](REWIND_v3.0.md)）。

## 4.0.0 の変更（2026-10-26）

コンパイラと言語の版を 4.0.0 に上げました。詳しくは[言語リファレンスの付録 B](language-reference.md#400-の変更)と[4.0.0 の変更点](REWIND_v4.0.md)にあります。

- 言語 4.0.0 では、宣言・制御・チェックポイント・式の約 45 語が予約語になり、名前に使えません。修飾語（`value capture borrow force live fresh test where effects captures`）は文脈依存のままです。
- 言語 4.0.0 では、ソースの `null` リテラルを書けません。内部の値は DB と JSON のために残ります。
- REWIND 4 は `language` 3.0.0 未満を受け付けません。テストの一部が、旧版の言語を使うため、環境変数 `REWIND_LEGACY_LANGUAGES`（`.cargo/config.toml` で設定、リリースには含めない）で旧版を動かしています。旧版の分岐コードとテストの整理は、これからの作業です。
- どの言語でも、通常の文字列リテラルの中の改行は LF になります。
- `binding.revert` を削除し、`historyGraph` の名前付き引数 `branch:` を `branchName` に改名しました。
- 型注釈つき `let` の短いクロージャ、末尾のカンマ、raw 文字列を追加し、`match E::A {` と `a < b >= c` を読めるようにしました。

- 作業とcommit/pushは`codex/develop`だけ。今回mainへ統合しない。
- 公開CI/CDを動かさない。commit messageに`[skip ci]`を付ける。
- workflow dispatch、release tag、Release開始message、Release作成をしない。
- 小さなreview可能なまとまりごとにcommit/pushして、未保存の大きな差分を残さない。
- 公開済みv2.0.0の仕様を出発点とし、既存機能を未実装として列挙しない。
- 2026-10-11に構想の実装を依頼された。CP-05/06の一部から着手し、compiler/libraryの変更を小さく保存する。

過去の「公開後mainへ統合」という一般方針は、今回の「codex/developだけ」という指示で上書きされています。将来の実装時も、当時の最新指示を確認してから公開を扱います。

## 2026-10-16の検証状況

履歴・変更操作などの標準module（`changes` `history` `historyGraph` `lazy` `lens` `memo` `orderedMap` `parser` `persistentSet` `schema` `snapshot` `textEdit` `textIndex` `timeline`）は、このstatusの時点ではsource実装のみでした。現在は標準ライブラリの全test（`rewind test libraries/std/tests.rw`）がLinuxとWindowsのCIで通り、公開APIのbaselineを追加して`api-diff --deny-breaking`の対象にしました。公開CI/CDの指示は、最新のユーザー指示（版を上げて公開してよい）で上書きされており、v3.1.0〜v3.4.0を公開しています。

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

DATA-01のJSON部分をstd.schemaへ追加した。入れ子object/array、型と範囲、default、明示NULL、unknown policy、typed pathを扱う。Schemaはdataとして保持でき、Decoderは明示mapperと失敗段階を持つ。CSV adapterとDB row/bounded batch schema adapterを実装。form adapter・DB/HTTP共通batchとderiveは未実装。

VCACHE-02/03/04/06/12の基本部分をstd.memoへ実装した。domain/code revision/inputを分けたkeyで複数版を保持し、entry数とcaller指定weightでLRU evictionする。weightを実測memoryと呼ばない。orderedMapへfirst/last/popFirst/popLast/touchを追加し、単一evictionで全keyを列挙する処理を避けた。

VCACHE-07/06の遅延値をstd.lazyへ実装した。peekは計算せず、forceで成功snapshotまたは回復可能なErrを保持する。Sendな可変ownerの結果も独立forkへ渡せる。memo/lazyは通常のVM stateであり、hyper領域や外部I/Oのcacheではない。

追加したstd test sourceは13件。typed-schema、versioned-cache、lazy-snapshotに利用例を保存した。compile・実行・API生成・Windows・GC/性能測定は省略。sourceの括弧対応と同名関数の重複を確認したが、実行検証に数えない。

保存先はcodex/developのみで、すべて[skip ci]付き。確認時点のmainは5f5ab8f37c4d21f8746b2d1db077946809e3be42のまま。Actions最新実行も2026-10-10の既存commitから増えていない。

## 2026-10-12のGUI-03テーブルrow ID追加分

`std.guiTable.createWithIds`は1..64 byteのunique caller IDsを受け取り、row ID/index lookup、IDによる選択、描画button eventのrow ID解決を提供する。既存`create`の`row:<index>`は互換用で、並べ替えを越えるidentityとは扱わない。このrow ID追加段階ではsort/filterと差分編集modelは未実装だった（差分overlayは後述節で追加）。実行・compile・API生成・native確認は省略した。

## 2026-10-12のGUI-03フォームrelation追加分

`std.guiForm.Relation`と`createWithRules`を追加した。最大128 rulesを作成時にfield ID・kind検証し、typed equality、整数大小、checkbox依存必須を`validate`とreadonlyな`toJson`へ適用する。入力中の文字は失敗時にも保持し、依存fieldが編集されたら該当relation errorを無効化する。

custom validator、呼出側recordへのtyped adapter、editable table、安定row ID/sort/filterは未実装。compile、実行、API生成、GUI backendは未確認。

## 2026-10-12のGUI-02 component ID追加分

`std.gui.ComponentScope`を追加した。scope nameは1..24 UTF-8 bytes、local IDは1..64 bytesでNULを拒否し、`componentId`はbyte長を含む安定prefixで組を一意に符号化する。`componentPrefix`は最大27 bytesで、`std.guiForm.render` / `dispatch`へ渡せる。scope自体はmodel、callback、native handleを所有しない。呼出側がVM stateとaction routingを担当する。

共有componentのrender/reducer、form/table wrapper、IME compositionを保つ再描画は未実装。例とAPI manifestを追加した。compile、実行、API生成、native backendは未確認。

## 2026-10-12のGUI-01レイアウト追加分

`std.gui`へnested row/column/overlay layout tree、weighted main-axis min/max/fixed sizing、atomicな`applyLayout`を追加した。flatな`splitRow`/`splitColumn`はweightとgapのみに対応し、個別min/maxは持たない。上限は1024 node、深さ32、containerあたり256 child、4096 pixel。領域が制約を満たせない場合はView更新前にerrorを返す。focus/edit stateはwidget ID単位で維持する。

GUI-01は部分実装で、overlay layout nodeは後続commitで実装した。padding、cross-axis constraints、自然寸法測定、resize時自動再配置を残す。API manifestと計画の記述をsourceに合わせた。環境起動失敗のためcompile、実行、API生成、native Windows/Linux動作は確認していない。source変更はcodex/developのみへ`[skip ci]`付きで保存し、mainと公開CI/CDへ触れない。

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
5. [言語リファレンス](language-reference.md)、[現行入門書](book/index.html)、[v2.0仕様](REWIND_v2.0.md)、[受入履歴](v2-acceptance.md)：現在の契約を確認する資料。

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

GUI-01〜03は基礎の一部を実装、他は未着手です。IDのGUI-01はv2.2詳細内の作業名であり、性能文書のGUI-01 caseとは区別して参照します。

| ID | 実装差分 | 依存・確認 | 完了または保留の判断 |
|---|---|---|---|
| GUI-01 | 入れ子row/column、main-axis min/max、atomic applyLayout | arrange、edit、composition | source保存済み。padding/cross-axis/自動resizeと実行確認は残る |
| GUI-02 | scoped child ID基盤 | layout ID、effect/ownership | source foundation saved; model/render/reducer and IME-preserving reuse remain |
| GUI-03 | form relations, typed mapper, caller row IDs, bounded cell-edit overlay | 既存form/table、GUI-01 | form/row-ID/model-edit foundations saved; native input/edit lifecycle and sort/filter remain |
| VIS-01 | 限定plot | renderer primitive、numeric view | 費用/互換確認。未成立なら保留可能 |
| DB-01 | 明示record mapping | typed cell/cursor/schema | validateRow/validateBatch source saved; NULL/duplicate/missing/precision and backend cleanup execution acceptance remains |
| DB-02 | transaction helper | external/await/affineの最小例 | 終了状態、cancel、unknown、cleanup |
| DB-03 | 限定migration | DB-02、backend lock/DDL | 同時適用、checksum、途中失敗 |
| APP-01 | 入力・一覧・保存の例 | 必要なGUI/DB単位 | 外部境界、失敗表示、shutdown |

DB transaction、form検証、table viewport、協調GUI入力を新規不足として扱わないこと。DB-02のcallback設計が所有権上成立しない場合は、明示handleを優先し、必要なcompiler差分を計画へ戻します。

## 性能段階

| ID | 状態 | 次の成果物 |
|---|---|---|
| PERF-01 | 未着手 | 共通測定formatとbaseline binary/input記録 |
| PERF-02 | 一部測定済 | Windowsで主要9 caseを前後比較（[記録](v2-next-perf-results.md)）。Linux・GUI・HTTP・PostgreSQL・IO-01・SCHED-01は未測定 |
| PERF-03 | 測定済・1件修正 | 固定rootの繰返しで説明のつかない増加は、スケジュール記録の無制限増加1件のみ（修正済み）。ledgerは回数に比例しない |
| PERF-04 | 3件改善済 | VM命令あたりの確保削減、非連続viewの走査、スケジュール記録。残り：予算確認の全状態走査、external台帳の走査（[記録](v2-next-perf-results.md)） |
| PERF-05 | 一部実施（ローカルの主要経路は完了） | VM/数値/task/checkpoint/recordは10,000 cycle以上で一定。SQLite loopも10,000 cycle完走（14秒、回数に比例。external台帳分の約1.2 KiB/cycleは設計どおり）。GUI・HTTP・PostgreSQLの長時間は未実施 |

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

DATA-04全体は未完了。CSVの1-row consumer、JSON event→schema、DBのbounded schema batchはsource実装済み。application recordへのmapper、DB/HTTP cursor共通化、cross-source batch APIは残る。queue管理・chunk再送・cancelは既存reader契約を使い、std.schemaは入力streamや外部資源を所有しない。


## 2026-10-12のDB column mapping helper

v2.2のDB-01に`std.db.ColumnMap`、`indexColumns`、`columnNumber`を追加した。cursor metadataの列名を最大256件まで一度検査し、重複名を拒否してimmutable Mapへ保持する。同じbatch内の各rowでname lookupを繰り返せる。record生成とInt/Text等の選択は既存typed accessorを使って呼出側に残した。

[SQLite](../examples/database/README.md)と[PostgreSQL](../examples/postgres/README.md)の両exampleで、backendを越えて同じColumnMap APIから列位置を引き、typed accessorを使うuser mapperがScore / StatusRowを構築する。既存の1件ずつ読むcursorを維持し、全rowを収集しない。新しいtest sourceは追加していない。実行環境のcontainer起動失敗によりcompile/runtime確認は未実施。SQLite/PostgreSQLの両方での実行受入、NULL/duplicate/missing、cleanup確認はDB-01の残り。

## JSON event streamの再開分

2026-10-12に`std.jsonStream.peek(&Reader)`と`nextBigInt(&mut Reader)`を追加しました。peekは先頭のqueued eventを検査してもreaderをadvanceしません。nextBigIntはJSON number tokenを正確な10進BigIntへ変換してから消費します。型違い・非整数・変換失敗ではeventをqueueに残し、別の方法で再解釈できます。

API manifest、library一覧、DATA-04の実装状況、利用例を更新しました：[json-stream](../examples/json-stream/README.md)、[BigInt例](../examples/json-stream/bigint.rw)。実装commitは[1f2052d](https://github.com/viral8code/REWIND/commit/1f2052d9cab174166f225ddd2bc984304306e253)と[e1b0fbf](https://github.com/viral8code/REWIND/commit/e1b0fbf17f9d6c3c556f8bb6df9139e0d41f709d)です。

DATA-04にはvalidated Jsonからapplication recordへ写す明示mapperと、DB cursor/HTTP sourceとの共通batch APIが残っています。source・例のcompile/runtime/API再生成・Windows確認は未実施です。commitはcodex/developのみへ`[skip ci]`付きで保存し、公開CI/CD、main統合、releaseは行っていません。


## DATA-04 JSON event materialization / schema validation

`std.jsonStream.nextJson`はJSON event列からnested object/arrayを含む一つのroot `Json`を構築する。`nextValidated`はshared `std.schema`で検証・default補完した値を返す。不完全chunkとparse・duplicate-key・schema errorではReaderのqueued eventを消費しない。上限は深さ64、object/array各65,536要素。number eventは通常のJSON parse経路(Int/Float)となるため、exact BigInt/Decimalは専用helperを使う。source、API manifest、split-input・validation-failureのtest source、[schema example](../examples/json-stream/schema.rw)を追加した。compile/runtime/API regenerationは未実施。

## DB-01のbounded row mapper

2026-10-12に`std.db.mapBatch`を追加しました。最大64行の既存`DbBatch`を借用し、pureな明示mapperで型付きrecordへ変換します。最初のmapping失敗はbatch内の0起点row indexとmapper errorを返します。失敗しても入力batch/cursorは変更しません。

[SQLite](../examples/database/README.md)と[PostgreSQL](../examples/postgres/README.md)の例はcursorの列metadataから`ColumnMap`を作り、`mapBatch`へmapperを渡します。DBの読込みは引き続き明示的な`db.next`で行います。API manifestへdevelop上のColumnMap APIと新しいgeneric batch APIを反映しました。

実装は[d8315c8](https://github.com/viral8code/REWIND/commit/d8315c8e70f2e15d3e45327cef4b163f1887b1c5)です。compile/runtime/API再生成とSQLite/PostgreSQL実接続の受入は未実施です。


## DB-01 / DATA-01 shared schema row and batch mapping

`std.db.validateRow`は最大256列をJSON objectへ写し、`std.schema`で検証する。`validateBatch`は列metadataの重複を一度検査し、最大64行を入力順に検証してschema適用済みの`MappedBatch<Json>`を返す。row errorはbatch内indexを含む。DbBatch/cursorは変更しない。Bytes/Private cellは暗黙変換せず、SQLite Decimal BLOBは明示的な`dbDecimal.cell` mapperを必要とする。

SQLite例をbounded batch APIの利用へ更新した。row shape、schema error、batch限界、元batch保持のtest sourceを追加した。compile/runtime/API generationとSQLite/PostgreSQL接続・cleanup確認は未実施。

## GUI-01のflat weighted split

2026-10-12に`std.gui.splitRow` / `splitColumn`、`arrangeRow` / `arrangeColumn)、nested layout treeを追加しました。treeは`leaf`、`weighted`、`row`、`column`で構成し、最大1024 node・深さ32・各container 256 childに制限します。weightは1..1,000,000、gapは0..4096 pixelです。

`applyLayout`はboundedなwork queueでnodeを走査し、全leaf ID・重複・Rect・View boundsを確認してからgeometryを更新します。失敗時にViewは変更せず、widgetのcaption・selection・focus indexは維持します。[利用例](../examples/gui-layout.rw)。実装は[1118325](https://github.com/viral8code/REWIND/commit/1118325534ca61a338bee1a72249e1f0fe0b0eed)です。

overlay node、padding、weighted childのcross-axis min/max制約は実装済みです。自然寸法測定、resize eventへの自動再配置は未実装です。compile/runtime/API再生成、Linux/Windows native GUI確認は未実施です。


## 2026-10-12のGUI-03 table staged-edit overlay

std.guiTableへeditCell、editCellId、hasEdits、changes、clearEditsを追加した。変更はstable row IDとcolumn indexで記録し、cellと既存renderはoverlayを表示する。基礎のFrozen cell matrixを複製せず、元値に戻したcellは差分から除く。最大4096 cell・値合計4 MiB・各value 4096 bytesで制限する。削除時はboundedな差分一覧を再indexする。

これはVM上のモデル編集で、stable text sort、bounded text filter、weighted column widths、stable IDを使うtyped row mapping、model-side draft lifecycleを後続の独立した実装分に追加した。native text input/IME eventとの接続、application固有validation、draftとGUI editorの完全な統合は残る。gui-table-edits.rwを追加した。環境起動失敗のためcompile、実行、API再生成、native GUI確認は行っていない。source/API/doc/exampleをcodex/developのみに保存し、公開CI/CD、main統合、Release、version更新は行わない。


## 2026-10-12のGUI-03 typed form adapter

std.guiFormへSubmitError<E>とtoValue<T,E>を追加した。toValueは既存toJsonの必須・型・範囲・relation検証を通し、成功したtyped Json objectだけをpureな利用者mapperへ一度渡す。form validation errorとrecord conversion errorを別variantで返し、form stateを変更しない。reflectionや暗黙変換は含めない。

gui-form-typed.rwに明示record mappingの例を追加した。API manifestとGUI-03計画を更新。環境起動失敗のためcompile、実行、API再生成は未確認。変更はcodex/developのみで[skip ci]付き、公開CI/CD、main統合、Release、version更新は行っていない。


## 2026-10-12のGUI-03 table stable sort

`std.guiTable.sortByText`で一列のString値を昇順/降順に安定sortする。descending=falseが昇順、trueが降順で、同値は現在の表示順を保つ。sort keyはstaged edit overlayを通して読む。8192行と256 UTF-8 byte/keyを上限にし、column/sizeエラーではtableの順序・選択・viewport・編集差分を変更しない。

表示順→base rowとbase row→表示位置のindexをTableに保持する。Frozenなcell matrixは並べ替え・複製せず、row ID、選択、staged editは同じbase rowに紐付く。rowIndex/selection/Action::SelectedのIntは現在の表示位置となる。実装とAPI、[利用例](../examples/gui-table-sort.rw)、GUI-03計画を更新した。commitは[a89ad45](https://github.com/viral8code/REWIND/commit/a89ad45b404a3158c8d6497d0582a671f37ad258)。

compile、実行、API再生成、native GUI確認はしていない。commitのActions実行は0件、mainは変更なし。commit/pushは`codex/develop`だけで`[skip ci]`付き。

## 2026-10-12のGUI-03 table text filter

`std.guiTable.filterByText(table, column, query)`は列の文字列を対象にcase-sensitive部分文字列filterを適用し、`clearFilter`で解除する。`rows()`は元データ件数、`visibleRows()`はfilter後の件数を返す。全体のsort順`rowOrder`と表示対象`displayOrder`を分けて保持するため、filter解除後もsort順が維持される。row IDとstaged editは元のbase rowに結び付く。

選択された行がfilterで隠れたら選択を解除し、隠れたIDの`rowIndex`はNoneを返す。`editCellId`は隠れた行にも適用でき、filter対象列の値を編集したときと`clearEdits`時に表示集合を再計算する。viewportと描画は可視行だけを走査する。非空filter queryは最大256 UTF-8 bytes、対象テーブルは最大8192行に制限する。

[利用例](../examples/gui-table-filter.rw)を追加し、GUI reference、API manifest、v2.2計画、実装indexを更新した。source/API/docs/example変更はcommit `a591995538e7323fa9ebcfa79ef78e83946cd398`。

コンパイル、実行、API再生成、native GUIは環境起動失敗のため未確認。commitのActions実行は0件、mainは`5f5ab8f37c4d21f8746b2d1db077946809e3be42`のまま。codex/developだけへ`[skip ci]`付きで保存し、Releaseと版更新は行っていない。

## 2026-10-12のGUI-03 table column weights

`std.guiTable.setColumnWeights`は1..16列それぞれに1..65535の正整数weightを設定する。既定値は全列1。描画時に32pxの左右insetを除いたinner widthを按分し、各列に最低1pxを割り当てて、余りを先頭列から配るため列間に空白や重なりが出ない。変更は次回renderから反映し、行・sort・filter・selection・編集差分には影響しない。weight件数違いと不正値はTableを更新する前に失敗する。

[利用例](../examples/gui-table-column-weights.rw)を追加し、GUI reference、v2.2計画の受入条件、実装index、API manifestを更新した。source/API/docs/exampleの実装commitは`41eb7ec18093776b826b552a68cf8e9f944a378e`。

コンパイル、REWINDのテスト、API再生成、native GUI確認は環境起動失敗のため未実施。source brace balanceと公開function/API symbol数は一致し、幅配分式を外部の静的計算で境界・ランダム入力確認した。これはcompiler/runtime検証の代わりではない。codex/developだけへ`[skip ci]`付きで保存し、公開CI/CD、main統合、Release、版更新は行っていない。

## 2026-10-12のGUI-03 typed table row adapter

`std.guiTable.toValue<T,E>`はstable row IDから1行の列値をcolumn orderのFrozen snapshotへ作り、利用側のpure mapperを1回呼ぶ。1回に最大16列のsnapshotだけを作るため、全tableをtyped recordに一括materializeしない。staged edit overlayを反映し、filterで非表示の行も既知IDなら変換できる。mapper errorは`RowMapError::Conversion(rowId,error)`として行IDと共に返し、unknown IDは`UnknownRow(rowId)`となる。変換はtable modelを変更しない。

[利用例](../examples/gui-table-typed.rw)、GUI reference、API manifest、v2.2計画、実装indexを更新した。実装commitは`599df4ef895c11ea4af95e172c1f7b36901a9d44`。

コンパイル・REWINDテスト・API再生成・native GUI確認は環境起動失敗のため未実施。source brace balanceと公開function/API symbol数、function名は一致することを確認した。これをcompiler/runtime検証として扱わない。codex/developだけへ`[skip ci]`付きで保存し、公開CI/CD、main統合、Release、版更新は行っていない。

## 2026-10-12のGUI-03 table edit draft lifecycle

`std.guiTable.beginEdit`は可視cellの現在値をoriginal/valueとして1つのdraftに保存する。`draft`と`setDraft`で値を確認・更新し、`commitEdit`の成功後に初めてstaged overlay、`cell`、`changes`、`toValue`へ反映する。`cancelEdit`はdraftだけを破棄する。確定時のedit limit/capacity failureは値を失わずdraftを開いたままにする。直接編集APIはdraftが開いている間`EditActive`で拒否する。draftはstable row IDと列番号に結び付くのでsort/filter後も対象行を失わない。

[利用例](../examples/gui-table-edit-lifecycle.rw)を追加し、GUI reference、v2.2計画、API manifest、実装indexを更新した。実装commitは`356baffbd4666590f542d859264cf4f654df763a`。

コンパイル・REWINDテスト・API再生成・GUI event/IME/native確認は環境起動失敗のため未実施。source brace balanceと公開function/API symbol名・数が一致することを確認しただけで、compiler/runtime検証ではない。codex/developだけへ`[skip ci]`付きで保存し、公開CI/CD、main統合、Release、版更新は行っていない。

## 2026-10-12のGUI-01 overlay layout node

`std.gui.overlayLayout(&layers)`は2..256個のchild Layoutを合成し、各layerへ親から渡された同一Rectを割り当てる。入力treeと合成結果は既存の1024 node・深さ32制限を守る。apply時には未知ID、重複ID、invalid boundsを全placement検証後にまとめて確認し、1件でも失敗すればView widgetを更新しない。layoutの適用はwidget insertion orderを変えないため、z-orderはViewへ追加した順で決まる。

[利用例](../examples/gui-layout-overlay.rw)を追加し、GUI reference、API manifest、v2.2計画、実装indexを更新した。実装commitは`143ed9ef76a61992a729769ee76fad4ea1bda288`。

コンパイル・REWINDテスト・API再生成・native GUIは環境起動失敗のため未確認。source brace balanceと公開function/API symbolの数・名前の一致のみを確認した。codex/developだけへ`[skip ci]`付きで保存し、公開CI/CD、main統合、Release、版更新は行っていない。

## 2026-10-12のGUI-01 padding layout node

`std.gui.padded(layout,left,top,right,bottom)`は四辺0..4096 pixelのinsetを受け取り、inner Rectへchild layoutを配置するnodeを作ります。構築時にtree node数と深さを検査し、適用時はinner width・heightにそれぞれ1 pixel以上を要求します。領域不足やID/bounds検証失敗ではViewを部分更新しません。layout depthはchildから引き継いで上限を判定します。

[利用例](../examples/gui-layout-padding.rw)、GUI reference、API manifest、v2.2計画、実装indexを更新しました。実装commitは[f72411b](https://github.com/viral8code/REWIND/commit/f72411b7247c316d30c3a2f50cc5783a695665f0)。

環境の起動失敗によりコンパイル・実行・API再生成・native GUI確認は未実施です。sourceのbrace balance、公開function/API symbol数、API JSON parse、padding depthの静的確認のみを行いました。codex/developだけへ`[skip ci]`付きでcommit/pushし、公開CI/CD、main統合、Release、版更新は行っていません。


## 2026-10-12のGUI-01 cross-axis制約

`std.gui.crossSized(child,min,max)`は`WeightedLayout`のcross axisを制約します。rowでは各childの高さ、columnでは幅に適用し、最小値を満たせないcontainerは`GuiLayoutInsufficientSpace`で失敗します。最大値より広い場合はchildを上端または左端へ揃え、余りを反対側へ残します。従来のmain-axis weight/min/maxと合成でき、flat split APIの既定geometryは変わりません。layout tree全体のplacement検証後にだけViewを更新します。

[利用例](../examples/gui-layout-cross-axis.rw)、GUI reference、API manifest、v2.2計画、実装indexを更新しました。実装commitは[d72a178](https://github.com/viral8code/REWIND/commit/d72a178fdf0c6ced88c0792d6725d233886f04ff)。

環境の起動失敗によりコンパイル・実行・API再生成・native GUI確認は未実施です。source brace balance、公開function/API symbol名と数、API JSON parse、layout node patternの静的確認のみを行いました。codex/developだけへ`[skip ci]`付きでcommit/pushし、公開CI/CD、main統合、Release、版更新は行っていません。


## DATA-04のJSON Decimal event

`std.jsonStream.nextDecimal`は先頭のJSON number eventを既存`std.decimal.parse`でexact Decimalへ変換し、成功後にだけreaderをadvanceします。event種別違い・parse失敗は`DecimalEventError`として返し、eventをqueueに残すため、Floatを経由せず再試行できます。BigInt用`nextBigInt`と同じconsume契約です。

[利用例](../examples/json-stream/decimal.rw)を追加し、jsonStream API manifest、DATA-04計画、実装index、library一覧を更新しました。実装commitは[a92bbf8](https://github.com/viral8code/REWIND/commit/a92bbf8528fcd7810418fd1ec5633ae847821362)。

環境の起動失敗によりコンパイル・実行・API再生成は未実施です。source brace/parenthesis balance、公開function/API symbol名の一致、API JSON parse、parse-before-advanceの静的確認のみを行いました。codex/developだけへ`[skip ci]`付きでcommit/pushし、公開CI/CD、main統合、Release、版更新は行っていません。


## DATA-01 exact Decimal schema

`std.schema.decimal()` validates exact Decimal text through the existing `std.decimal.parse`. JSON input uses `Json::Text` so an earlier Float conversion cannot discard precision; CSV uses the raw cell text. Successful validation retains the supplied representation, including scale. Invalid input returns a value-free `ValidationError`; DB/form and date-time rules remain open.

Added an acceptance source test for exact scale, malformed text, non-Text numeric values, and CSV mapping, plus [an example](../examples/typed-schema-decimal.rw). Updated the schema implementation contract, DATA-01 plan, implementation index, and library list. Implementation commit: [086fc04](https://github.com/viral8code/REWIND/commit/086fc04c676bd57b83be05b8313db7d01a8657b6).

The container failed to start, so compiler, runtime, test, and API generation checks are unavailable. Static checks covered node match exhaustiveness by source inspection, delimiter balance, and preservation of API input text. No `schema.api.json` existed on this branch; public API inventory is in `libraries/README.md`. Changes are pushed only to `codex/develop` with `[skip ci]`; no public CI/CD, main merge, Release, or version update was run.


## DATA-01 RFC3339 instant schema

`std.schema.timestamp()` validates JSON Text and CSV raw cell text with `std.datetime.parse`. It requires an explicit RFC3339 offset and inherits the existing parser's calendar/range, nanosecond precision, and leap-second rules. Validation preserves the supplied string; callers explicitly convert it with `std.datetime.fromJson`. Errors do not include the input value. Local calendar and timezone ambiguity rules remain separate.

Added [a timestamp schema example](../examples/typed-schema-timestamp.rw) and acceptance source coverage for fractional precision, offset, invalid date, and CSV mapping. Updated the schema reference, DATA-01 plan, implementation index, and library list. Implementation commit: [8305b7d](https://github.com/viral8code/REWIND/commit/8305b7d74f845588d64ee1e44cace6eb1b382522).

Compiler/runtime/API generation remain unverified because the execution environment failed to start. Status recorded on codex/develop with `[skip ci]`; no public CI/CD, main merge, Release, or version update was run.


## GUI-03 shared schema validation for forms

`std.guiForm.toSchemaValue` applies form field/relation checks, then a reusable `std.schema`, then one pure application mapper. `SchemaSubmitError` separates form, schema, and conversion failures. Validation is read-only; the mapper does not run until both validation stages succeed.

Added [an example](../examples/gui-form-schema-validation.rw) and an acceptance source test for schema rejection, unchanged form text, and successful mapping. Updated the GUI reference, GUI-03 plan, API manifest, library list, and implementation index. Implementation commit: [81d7780](https://github.com/viral8code/REWIND/commit/81d7780ab1df3e35ee9d6acdc244703f76fa1bcc).

Compiler, runtime, tests, and API generation remain unverified because the execution environment failed to start. Static checks covered source/API function names, JSON parse, and delimiter balance. The feature and this record are on `codex/develop` with `[skip ci]`; no public CI/CD, main merge, Release, or version update was run.


## GUI-03 shared schema validation for forms

`std.guiForm.toSchemaValue` applies form field/relation checks, then a reusable `std.schema`, then one pure application mapper. `SchemaSubmitError` separates form, schema, and conversion failures. Validation is read-only; the mapper does not run until both validation stages succeed.

Added [an example](../examples/gui-form-schema-validation.rw) and an acceptance source test for schema rejection, unchanged form text, and successful mapping. Updated the GUI reference, GUI-03 plan, API manifest, library list, and implementation index. Implementation commit: [79c5540](https://github.com/viral8code/REWIND/commit/79c55409948bd5d02591b351768de54686a0f104).

Compiler, runtime, tests, and API generation remain unverified because the execution environment failed to start. Static checks covered source/API function names, API JSON parse, and delimiter balance. The feature and this record are on `codex/develop` with `[skip ci]`; no public CI/CD, main merge, Release, or version update was run.

2026-10-19: std.html / std.template 追加（版は上げない）。`std.html` は HTML の escape・安全な URL・属性を検査した element 構築を、`std.template` は JSON data の `{{name}}` 描画を、pure な `.rw` として `libraries/std` に追加した。API baseline（`libraries/api/html.api.json`、`template.api.json`）、`libraries/README.md`、`docs/language-reference.md` に反映した。コミット・push・版の更新はしていない。

2026-10-19: std.base64 / std.auth 追加（版は上げない）。`std.base64` は標準と URL-safe の encode/decode を、`std.auth` は PBKDF2 によるパスワード保存と検証、署名付き token、CSRF token を、pure な `.rw` として `libraries/std` に追加した。salt と key は呼出側が渡す（CSPRNG は未提供）。API baseline（`libraries/api/base64.api.json`、`auth.api.json`）、`libraries/README.md`、`docs/language-reference.md` に反映した。コミット・push・版の更新はしていない。
2026-10-20: std.bits / std.bytes 追加（版は上げない）。`std.bits` に leadingZeros・trailingZeros・rotateLeft/Right・reverseBits・byteSwap・extract・deposit を、`std.bytes` に concat・fromInts・toInts・indexOf・equal・readInt を追加した（native は `src/v2/v092.rs`）。API baseline（`libraries/api/bits.api.json`、`bytes.api.json`）、`libraries/README.md`、`docs/language-reference.md` に反映した。コミット・push・版の更新はしていない。
2026-10-20: std.crypto.randomBytes 追加（版は上げない）。OS の乱数源から 1〜65536 バイトを返す。`external` と `random` の権限が必要で、実行時に記録され replay で再生される（native は `src/v2/v092.rs`、API baseline は `libraries/api/crypto.api.json`）。
2026-10-20: std.fastx の大きな入力 (1 レコード 1M 塩基) で formatFasta を文字単位の切り出しからバイト単位の切り出しに変えた結果、width 60 の書き出しは 15.94 秒から 1.05 秒になった (250K: 1.19 → 0.36 秒)。parseFasta は行末の検出を native の stdBytesIndexOf に変えた結果、1M 塩基で 19.30 秒から 3.91 秒になった (250K: 4.93 → 1.21 秒、500K: 9.73 → 1.89 秒)。parseFastq は 250K 塩基で 10.69 秒から 0.36 秒、500K 塩基で 21.17 秒から 0.44 秒になった (400K 塩基 0.38 秒。1M 塩基は 1 MiB を超えるため測定対象外)。いずれも線形。
2026-10-20: std.guiTabs / std.guiCombo / std.guiDate 追加（版は上げない）。タブ、上限付きの選択式リスト、1900〜2200 年の日付選択を純粋な .rw で実装し、gui の画面へ重ねて描画する。
2026-10-20: std.binding 追加（版は上げない）。1 つの record の値と、欄の id つきの文字列の読み書きを束ね、版番号と差分で画面の再描画を判断する。guiForm との受け渡し、取り消し・やり直しを純粋な .rw で実装する。
2026-10-21: std.guiCombo の文字入力による検索を、大文字小文字を区別しない形に変更（版は上げない）。候補の順序と返す値は変えず、`std.unicode.lower` で小文字にそろえて比べる。`examples/gui-controls2.rw` に guiTabs / guiCombo / guiDate / binding を使う例を追加した。
2026-10-21: std.text に codePoints / fromCodePoints / lineSeparator を追加し、std.binding に reset を追加した（版は上げない）。lineSeparator は実行環境の改行を外部の観測として記録し、replay は記録した値を返す。binding.revert は reset と同じ動作のまま非推奨にし、4.0.0 で削除する予定。
