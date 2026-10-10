# 自由構想を組み合わせたsketch

以下は未実装APIを組み合わせる設計上の想像です。code fence内は擬似コードで、現行REWINDの実行例ではありません。

[533項目の入口](../README.md)から、利用者が何をするか、値がいつ確定するか、何を戻すかまで掘り下げます。独立した追加機能として件数へ重ねて数えません。

- [複数案を比べて採用する設定editor](01-trial-editor.md)
- [部分失敗を残さないstream import](02-atomic-import.md)

- [曖昧な入力を探索するparser](03-parser-exploration.md)
- [cellの結果を版として持つnotebook](04-versioned-notebook.md)

- [外部操作を組み立てるworkbench](05-effect-workbench.md)
- [taskの実行順を探索するlaboratory](06-scheduler-laboratory.md)

- [複数の未来を比べるsimulation tree](07-simulation-tree.md)
- [寿命を閉じ込めるnative計算capsule](08-native-computation-capsule.md)

- [編集途中も失わない型付きform](09-typed-form.md)
- [仮説と観測を並べる推論explorer](10-inference-explorer.md)
