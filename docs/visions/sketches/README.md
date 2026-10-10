# 自由構想を組み合わせたsketch

以下は未実装APIを組み合わせる設計上の想像です。code fence内は擬似コードで、現行REWINDの実行例ではありません。

[605項目の入口](../README.md)から、利用者が何をするか、値がいつ確定するか、何を戻すかまで掘り下げます。12のsketchを置きます。独立した追加機能として件数へ重ねて数えません。

## 利用体験とstateの分担

| 体験 | 主に戻すdata | 別に残すstate・扱う境界 |
|---|---|---|
| [複数案を比べて採用する設定editor](01-trial-editor.md) | 設定model | 候補catalog、試行数、data版としてのUndo |
| [部分失敗を残さないstream import](02-atomic-import.md) | batch内のcollection更新 | report、入力消費、DB transaction |
| [曖昧な入力を探索するparser](03-parser-exploration.md) | cursorと構文候補 | 失敗説明、tape、仕事量 |
| [cellの結果を版として持つnotebook](04-versioned-notebook.md) | 計算modelと採用結果 | run catalog、code/input版 |
| [外部操作を組み立てるworkbench](05-effect-workbench.md) | draftと未実行plan | receipt、Unknown、補償操作 |
| [taskの実行順を探索するlaboratory](06-scheduler-laboratory.md) | 仮想task群のstate | failure catalog、探索範囲、実入力との対応 |
| [複数の未来を比べるsimulation tree](07-simulation-tree.md) | worldと仮の時間 | 試行budget、観測版、実操作 |
| [寿命を閉じ込めるnative計算capsule](08-native-computation-capsule.md) | VMへ採用した結果data | region、borrow、native完了とcleanup |
| [編集途中も失わない型付きform](09-typed-form.md) | 編集modelのdata版 | 入力buffer、競合、保存receipt |
| [仮説と観測を並べる推論explorer](10-inference-explorer.md) | 推論data state | metrics、観測版、推論の保証範囲 |
| [domain言語を育てるworkspace](11-domain-language-workspace.md) | source/AST/評価data | origin、生成器版、型とeffectの検査 |
| [実行中のmoduleを版として更新する](12-live-module-evolution.md) | 移行可能なmodel data | 旧frame、opaque型、physical owner |

「戻す」は各modelのdata操作を表します。すべての操作が現行revert構文で行える、あるいは外部stateも戻るという意味ではありません。

## 組合せから見えた選択

[設計の軸と比較scenario](design-questions.md)に、実行checkpointとdata snapshot、retained state、begin、外部作用、cleanup、互換性等の12論点を残します。未決の選択肢で、現行仕様は変更しません。
