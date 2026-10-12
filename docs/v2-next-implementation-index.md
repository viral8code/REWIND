# 構想から実装した機能の入口

更新日：2026-10-12。対象：codex/develop。**実装sourceを保存済み。実行検証はユーザーの指示により省略。** 公開版の仕様・動作保証・全構想の完了表ではない。

## 今回の追加

| module / 変更 | 内容 | 草案との対応 | 利用例 |
|---|---|---|---|
| std.snapshot / std.history | data rootの保存・分岐、名前付きregistry | CP-05/06の基本部分 | [data-history](../examples/data-history.rw) |
| std.timeline | bounded Undo/Redo、seek、Redo破棄、古いrootの保持 | CP-05/06の利用シナリオ | [snapshot-timeline](../examples/snapshot-timeline.rw) |
| std.changes | String Mapの差分・逆差分・合成・検査付き適用・3方向merge・rebase preview | VPERSIST-08、VHISTORY-04/05/06の一部 | [map-changes](../examples/map-changes.rw)、[rebase](../examples/rebase-preview.rw) |
| std.historyGraph | 最大16親、branch HEAD、annotation、祖先・merge base・経路・predicate query、pinと明示prune | VHISTORY-01/02/03、07/08/09/10/11の一部 | [history-graph](../examples/history-graph.rw)、[整理とquery](../examples/history-maintenance.rw) |
| std.parser | byte cursor、合成・先読み・検査・span・左右結合 | VPARSE-01/02/03/05/07の基本部分 | [parser-combinators](../examples/parser-combinators.rw)、[演算式](../examples/parser-expression.rw) |
| std.orderedMap | 挿入順とlookup、両端操作、touch、snapshot | VPERSIST-04のString key部分 | [insertion-ordered-map](../examples/insertion-ordered-map.rw) |
| std.textIndex | 原文を保持するbyte/scalar/grapheme位置、境界変換、slice | VTEXT-01の3単位 | [text-positions](../examples/text-positions.rw) |
| std.lens | typed getter/setter、欠損path、合成、検査付き変更、有限の法則確認 | VOPTIC-01/02/04/06/10の基本部分 | [typed-lens](../examples/typed-lens.rw) |
| std.persistentSet | 任意版からのunion、複数版query、native page共有 | VPERSIST-06 | [persistent-union-find](../examples/persistent-union-find.rw) |
| std.textEdit | 原文を保つ置換、旧新位置mapping、編集列合成、挿入affinity、逆編集 | VTEXT-01/06の基本部分 | [text-edit-mapping](../examples/text-edit-mapping.rw)、[編集列](../examples/text-edit-chain.rw) |
| std.db row mapping | ColumnMap, MappedBatch<T>, RowMapError<E>, indexColumns, columnNumber, mapBatch | 列名index再利用と、最大64行のpureなrecord mapping。最初のerrorをbatch内index付きで返し、DbBatch/cursorを変更しない | DB-01の明示mapping用helper | [SQLite](../examples/database/README.md)、[PostgreSQL](../examples/postgres/README.md) |
| std.gui layout | Layout, WeightedLayout, leaf, weighted, sized, crossSized, row, column, overlayLayout, padded, applyLayout, splitRow, splitColumn, arrangeRow, arrangeColumn | 最大32階層・1024 node・256 childのnested row/column/overlay/padding tree、main-axis min/max/fixed size、atomic widget update | GUI-01のlayout tree基盤。overlayは全childへ同一boundsを適用し、paddedは四辺insetを検証してinner boundsへ適用。自然寸法測定・resize policyは未実装 | [gui-layout](../examples/gui-layout.rw)、[gui-layout-overlay](../examples/gui-layout-overlay.rw)、[gui-layout-padding](../examples/gui-layout-padding.rw)、[gui-layout-cross-axis](../examples/gui-layout-cross-axis.rw) |
| std.gui component IDs | ComponentScope, componentScope, componentPrefix, componentId, componentLocalId | byte長prefixによるscope/local IDの一意符号化。`componentLocalId`はActionから得たwidget IDをscopeで検査し、validなlocal action名へ戻す | GUI-02のID namespaceとscope-aware action routing helper。stateful component model/render/reducerは未実装 | [gui-component-scope](../examples/gui-component-scope.rw) |
| std.guiForm | createWithRules, toValue, toSchemaValue | 128 relation、form typed JSON、共通schemaを通してからpure mapperを呼ぶ。Form/Schema/Conversion errorsを区別 | GUI-03のform validation pipeline。custom callback validatorは未実装 | [gui-form-relations](../examples/gui-form-relations.rw)、[gui-form-typed](../examples/gui-form-typed.rw)、[gui-form-schema-validation](../examples/gui-form-schema-validation.rw) |
| std.guiTable row IDs | createWithIds, rowId, rowIndex, selectId, selectionId | caller-supplied unique 1..64 byte IDsでrow actionとdata rowを結ぶ。rowIndex/selectionは現在のdisplay positionを返し、filtered rowはrowIndex=None | GUI-03のrow identity foundation。sort/filter後も同じrow IDを維持 | [gui-table-row-ids](../examples/gui-table-row-ids.rw) |
| std.guiTable edit overlay | CellEdit, editCell, editCellId, hasEdits, changes, clearEdits | row ID + column indexでstaged差分を保持。Frozen base cellsを複製せず、4096 dirty cells / 4 MiB value bytesでbounded | GUI-03のmodel-side cell editing。sort後も差分を保ち、hidden rowもstable IDで編集。filter列の編集とclearEdits後に表示対象を再計算。native input、typed adapter、lifecycleは未実装 | [gui-table-edits](../examples/gui-table-edits.rw) |
| std.guiTable stable sort | sortByText | 文字列列を昇順/降順で安定sort。8192行・256 UTF-8 byte/keyまで。row-order indexのみ変更 | GUI-03の全行sort基盤。row ID・selection・edit overlayを維持 | [gui-table-sort](../examples/gui-table-sort.rw) |
| std.guiTable text filter | filterByText, clearFilter, visibleRows | Case-sensitive substring filter。最大8192 source rows・256 UTF-8 byte query。rowsはtotal、visibleRowsは表示件数。hidden selectionを解除し、filter列の編集後に再評価 | GUI-03のbounded table filter。native input event wiringは未実装 | [gui-table-filter](../examples/gui-table-filter.rw) |
| std.guiTable column weights | setColumnWeights | 1..16列に1..65535の正weightを設定。render時にinner width全体を按分し、各列幅を1px以上にする。設定変更は失敗時atomic | GUI-03のtable sizing。値なしは等幅。編集event lifecycleは別途 | [gui-table-column-weights](../examples/gui-table-column-weights.rw) |
| std.guiTable typed row mapper | toValue<T,E>, RowMapError<E> | stable IDの1 rowを最大16列のfrozen snapshotへ写し、pure mapperへ渡す。staged edit適用、filter hidden行対応。Conversionはstable IDとEを保持し、tableを変更しない | GUI-03のapplication record adapter。全件materializeを避け、1行ごとの明示変換 | [gui-table-typed](../examples/gui-table-typed.rw) |
| std.guiTable edit draft | beginEdit, draft, setDraft, commitEdit, cancelEdit | 一度に1 cell。draftはstable row ID + column + original/valueを保持し、commit成功までcell/changes/toValueへ反映しない。staging容量失敗時はdraftを維持 | GUI-03のmodel-side edit lifecycle。native text input/event/IME wiringとapplication validation integrationは未実装 | [gui-table-edit-lifecycle](../examples/gui-table-edit-lifecycle.rw) |
| std.csvStream | nextTyped | 既存CSV readerとobject schemaを1 row単位で接続、Validation failure時は未消費 | DATA-04のCSV row consumer | [csv-stream](../examples/csv-stream/README.md) |
| std.jsonStream | peek, nextBigInt, nextDecimal | 非消費event検査、JSON数値tokenをBigInt/Decimalへ正確に変換してから消費 | DATA-04のtyped stream前段。record materializationは未実装 | [json-stream](../examples/json-stream/README.md)、[BigInt](../examples/json-stream/bigint.rw)、[Decimal](../examples/json-stream/decimal.rw) |
| std.schema | 入れ子JSON検証、default/NULL/unknown policy、exact Decimal text、RFC3339 instant text、型付きmapper、header付きCSV row | DATA-01のJSON/CSV row部分。Decimal/Timestampはraw text parserを使いFloatを経由しない。DB/form・local date/timezoneは未実装 | [typed-schema](../examples/typed-schema.rw)、[typed-schema-decimal](../examples/typed-schema-decimal.rw)、[typed-schema-timestamp](../examples/typed-schema-timestamp.rw)、[typed-csv-row](../examples/typed-csv-row.rw) |
| std.memo | 版付きkey、LRU、件数/weight制限、pure Errのcache、選択失効 | VCACHE-02/03/04/06/12の基本部分 | [versioned-cache](../examples/versioned-cache.rw) |
| std.lazy | 明示force、成功snapshotとErrの保持、独立fork、clear | VCACHE-07/06の基本部分 | [lazy-snapshot](../examples/lazy-snapshot.rw) |
| generic Frozenの型判定 | T:SendからFrozen<T>のShareを導出し、呼出し・recordへ伝播 | 上記libraryの前提修正 | [記録](v2-next-frozen-bounds-implementation.md) |
| 入れ子Frozenのruntime修正 | 明示freezeの一段追加と、thaw時の内側Frozen保持 | snapshot/ChangeSetの合成を支える修正 | [記録](v2-next-nested-frozen-implementation.md) |

snapshot/historyの後に追加した12 moduleは合計201のpublic functionを持つ。既存std.csvStreamへのnextTyped追加はこの数に含めない。標準module registryに登録し、standalone importとSDK source生成の経路に接続した。署名付きSDK生成・API snapshot生成は未実行。

## 読む資料

- [snapshot / historyの契約](v2-next-snapshot-implementation.md)
- [timeline / changes / historyGraph / orderedMapの契約・費用](v2-next-data-tools-implementation.md)
- [parserの契約・費用](v2-next-parser-implementation.md)
- [text位置型の契約・費用](v2-next-text-index-implementation.md)
- [文字編集・位置mappingの契約](v2-next-text-edit-implementation.md)
- [lensの契約・費用](v2-next-lens-implementation.md)
- [schemaと型付きdecoderの契約](v2-next-schema-implementation.md)
- [版付きmemo cacheの契約・費用](v2-next-memo-implementation.md)
- [遅延値と結果snapshotの契約](v2-next-lazy-implementation.md)
- [永続union-findの契約・費用](v2-next-persistent-set-implementation.md)
- [状態と保存履歴](v2-next-status.md)

例は新しい実行ファイルでrewind run examples/FILE.rwとして利用する想定。公開済みv2.0.0 binaryに新moduleが同梱されたという意味ではない。各例のexpected出力は期待値であり、実測結果ではない。

## 確認sourceと未確認範囲

継続分ではlibraries/std/tests.rwへ52 test、tests/generic_frozen_bounds.rsへ1 testを追加した。compile、runtime、source-free artifact、record/replay、Windows、format、API生成、性能・GC測定は今回実行していない。source読解とGitHub上の差分・ref確認を、これらの実行検証の代わりとして数えない。

runtimeのFrozen修正は既存artifact/traceへ影響し得るため、次回検証する場合は通常のgeneric/ownership確認だけでなく、入れ子値とrecord/replayも確認する。

## 既存実装を使う部分

VTEXT-03のchunk間UTF-8 decoderの基本部分は、既存std.streamのutf8/decodeにある。未完の最大3 byteを保持し、不正UTF-8と未完了を区別する。この基本decoderを新規の欠落機能として再実装しない。global byte offsetや他encoding等の発展部分は別の差分として扱う。

## 今回含まない部分

- 動的な実行checkpoint handle、continuation復元、checkpoint外のhyper領域。
- historyのsquash、annotation stream、跨runtimeの履歴pack。pruneとprotected rootの基本部分は実装sourceを追加した。
- 任意Tの自動diff/merge、domain不変条件の自動生成、外部作用のrebase。
- parserの左再帰、stream追加待ち、lossless/incremental tree、parse forest、error recovery。
- partial型のfile合成、native arena/ヒープ外memory、trait object、const generic等の独立したcompiler/runtime変更。

これらは別の到達条件を持つ。12 moduleを追加したことを629構想の一括採用・完了と扱わない。次に進む単位は、公開APIの新設だけでなく、現在のsourceと既存contractへの必要な差分を確認して選ぶ。

保存先はcodex/developだけ。各commitに[skip ci]を付け、公開CI/CD、main統合、tag、Release、版更新は行わない。
