# REWIND v1.4.0

GUI の編集モデルと外部の入力・変換状態を分離し、文字入力と配置を整備する。ウィンドウへの表示は従来どおり present / publish で確定する。

## 実装内容

- `std.gui.textBox` / `textArea`、Unicode scalar 単位の cursor・選択、挿入、Backspace / Delete、選択移動、Home / End、複数行の上下移動、Ctrl+A。
- Windows の確定 UTF-16 文字と Linux XIM の確定 UTF-8 文字を入力 journal へ渡す。IME の preedit は OS 側の状態として保持し、checkpoint の対象にしない。
- `Rect` / `arrange` / `resize` で配置を再計算。配置は IDs と寸法を先に検証し、失敗時は変更しない。
- textarea のスクロールと caret の可視化、選択範囲の表示、widget 内での clipping。
- `pollEvent` による非ブロッキング入力。入力がない観測も記録するため replay で polling の結果が再現される。短い処理単位の間で入力を処理できる。別 task の自動実行をこの API だけで保証しない。
- Windows の paint callback は scene を Arc で参照し、paint ごとの全 scene の深いコピーを省く。
- GUI 観測の上限を 100 万件へ拡張し、従来の履歴予算も適用する。trace 自体の上限も引き続き適用する。
- `revert begin` に pending expression の検査を追加し、消失した演算 operand による後続エラーを境界で診断する。
- メモ帳の Unicode 入力、保存、resize、Undo、record / replay、source-free 配布を検証する。

## 利用

[GUI guide](gui.md)、[言語リファレンス](language-reference.md)、[v1.5 草案](REWIND_v1.5.md)、[後続計画](ROADMAP_v2.md) を参照。

```sh
rewind run main.rw --allow-effects gui,fileRead,fileWrite
```

## 保証範囲

単一 canvas と基本的な文字編集・配置を対象にする。clipboard、ネイティブメニュー、file dialog、複数ウィンドウ、native Wayland、macOS、アクセシビリティ連携は今後の対象。選択と編集は Unicode scalar 単位であり、結合文字を grapheme 単位へ束ねる編集は未対応。文字表示と IME の利用には OS 側のフォント・入力サービスが必要。確定済みファイルは Undo しない。
