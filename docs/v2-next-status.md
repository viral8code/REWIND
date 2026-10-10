# v2.1以降：作業状態と再開手順

更新日：2026-10-10。今回の成果は**計画の文書化**です。v2.1以降のruntime/library実装、測定、版更新、Release公開はまだ行っていません。

## この作業で守る指示

- 作業とcommit/pushは`codex/develop`だけ。今回mainへ統合しない。
- 公開CI/CDを動かさない。commit messageに`[skip ci]`を付ける。
- workflow dispatch、release tag、Release開始message、Release作成をしない。
- 小さなreview可能なまとまりごとにcommit/pushして、未保存の大きな差分を残さない。
- 公開済みv2.0.0の仕様を出発点とし、既存機能を未実装として列挙しない。
- 今の依頼は計画追加。以下の実装手順は、実装を依頼されたときの再開先。

過去の「公開後mainへ統合」という一般方針は、今回の「codex/developだけ」という指示で上書きされています。将来の実装時も、当時の最新指示を確認してから公開を扱います。

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

まず既存APIとの差を最小例で確認します。採用した項目だけ、対象版・依存・受入を決めて実装一覧へ追加します。追加構想の保存履歴はgit log -- docs/v2-next-*.mdで確認できます。今回はruntime/libraryを変更していません。

## 実現難度で絞らない自由構想集

[docs/visionsの入口](visions/README.md)に、16分野・248項目を追加しました。言語、時間軸、GUI、データ、数理、AI、分散、hardware、検証、開発環境、高度algorithm、domain library、interaction、実験的計算、simulationを扱います。

動的commit名から、文字列lookup、opaque handle、型付きsnapshot、複数版の値へ掘り下げた[具体案](v2-next-checkpoint-api.md)も追加しています。

これらは**発想を保存する段階**です。既存58候補の採用判断・実装状態・版の到達条件は変更していません。既存機能の発展形も含むため、この248項目を「現行版にない機能」の一覧として転記しません。

保存履歴はgit log -- docs/visionsで確認できます。分野ごとにcommit/pushしました。runtime/library変更、Release、版更新、main統合、公開CI/CDの起動操作は行っていません。

今後さらに思い付いた場合は、対応する分野へ新しいIDを付け、入口の件数を更新します。採用へ進める項目だけ、現行実装との照合と設計・受入を後で行います。

## 最初の実装再開：CLI-01

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
