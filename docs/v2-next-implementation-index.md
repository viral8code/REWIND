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
| std.schema | 入れ子JSON検証、default/NULL/unknown policy、型付きmapper | DATA-01のJSON部分 | [typed-schema](../examples/typed-schema.rw) |
| generic Frozenの型判定 | T:SendからFrozen<T>のShareを導出し、呼出し・recordへ伝播 | 上記libraryの前提修正 | [記録](v2-next-frozen-bounds-implementation.md) |
| 入れ子Frozenのruntime修正 | 明示freezeの一段追加と、thaw時の内側Frozen保持 | snapshot/ChangeSetの合成を支える修正 | [記録](v2-next-nested-frozen-implementation.md) |

追加した10 moduleは合計170のpublic functionを持つ。標準module registryに登録し、standalone importとSDK source生成の経路に接続した。署名付きSDK生成・API snapshot生成は未実行。

## 読む資料

- [snapshot / historyの契約](v2-next-snapshot-implementation.md)
- [timeline / changes / historyGraph / orderedMapの契約・費用](v2-next-data-tools-implementation.md)
- [parserの契約・費用](v2-next-parser-implementation.md)
- [text位置型の契約・費用](v2-next-text-index-implementation.md)
- [文字編集・位置mappingの契約](v2-next-text-edit-implementation.md)
- [lensの契約・費用](v2-next-lens-implementation.md)
- [schemaと型付きdecoderの契約](v2-next-schema-implementation.md)
- [永続union-findの契約・費用](v2-next-persistent-set-implementation.md)
- [状態と保存履歴](v2-next-status.md)

例は新しい実行ファイルでrewind run examples/FILE.rwとして利用する想定。公開済みv2.0.0 binaryに新moduleが同梱されたという意味ではない。各例のexpected出力は期待値であり、実測結果ではない。

## 確認sourceと未確認範囲

継続分ではlibraries/std/tests.rwへ45 test、tests/generic_frozen_bounds.rsへ1 testを追加した。compile、runtime、source-free artifact、record/replay、Windows、format、API生成、性能・GC測定は今回実行していない。source読解とGitHub上の差分・ref確認を、これらの実行検証の代わりとして数えない。

runtimeのFrozen修正は既存artifact/traceへ影響し得るため、次回検証する場合は通常のgeneric/ownership確認だけでなく、入れ子値とrecord/replayも確認する。

## 既存実装を使う部分

VTEXT-03のchunk間UTF-8 decoderの基本部分は、既存std.streamのutf8/decodeにある。未完の最大3 byteを保持し、不正UTF-8と未完了を区別する。この基本decoderを新規の欠落機能として再実装しない。global byte offsetや他encoding等の発展部分は別の差分として扱う。

## 今回含まない部分

- 動的な実行checkpoint handle、continuation復元、checkpoint外のhyper領域。
- historyのsquash、annotation stream、跨runtimeの履歴pack。pruneとprotected rootの基本部分は実装sourceを追加した。
- 任意Tの自動diff/merge、domain不変条件の自動生成、外部作用のrebase。
- parserの左再帰、stream追加待ち、lossless/incremental tree、parse forest、error recovery。
- partial型のfile合成、native arena/ヒープ外memory、trait object、const generic等の独立したcompiler/runtime変更。

これらは別の到達条件を持つ。10 moduleを追加したことを629構想の一括採用・完了と扱わない。次に進む単位は、公開APIの新設だけでなく、現在のsourceと既存contractへの必要な差分を確認して選ぶ。

保存先はcodex/developだけ。各commitに[skip ci]を付け、公開CI/CD、main統合、tag、Release、版更新は行わない。
