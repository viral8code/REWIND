# REWIND v1.9.7

## GUI の grapheme 編集

`gui.graphemeEditing(&mut view, true)` は textBox / textArea の編集を Unicode extended grapheme cluster 単位に切り替える。結合文字、ZWJ 絵文字、旗を Left / Right / Backspace / Delete で分割しない。Shift は cluster 単位で選択し、上下移動は行内の cluster 数を使う。CRLF はひとつの改行 cluster として扱う。

既定値は false で、以前の Unicode scalar 編集を維持する。`selection` / `setSelection` と scene の cursor / anchor は引き続き scalar offset である。モードの有効化時には既存の位置を次の cluster 境界へ揃える。有効化後の `setSelection` は cluster 内部の位置を `GuiInvalidSelection` で拒否し、値を変更しない。挿入や削除で隣接 cluster が結合した場合にも境界を再計算する。`setText` は従来どおり末尾へ移動する。

View 内の切替フラグと編集結果は VM の状態であり、commit / revert の対象になる。外部 window の描画や確定入力を巻き戻す保証は変更しない。確定前の composition、文字 shaping、描画上の cluster 幅、アクセシビリティは本版の追加範囲に含めない。4096 UTF-8 byte の入力上限は維持する。

```javascript
import std.gui as gui;
// view は gui.window の結果から取得する。
gui.graphemeEditing(&mut view, true);
```

実行可能な例は [gui-grapheme](../examples/gui-grapheme/main.rw)。標準 API は追加のみで、private View の内部 field は公開 constructor に含まれない。

## 検証

native 編集テストは結合文字、ZWJ、旗、Indic cluster、CRLF、上下移動、選択、挿入後の再分割、容量、無効位置と旧編集の動作を確認する。CLI と展開後 SDK では source-free artifact、checkpoint / revert と debug / compact replay を確認する。公開には既存 GUI / DB / network を含む全回帰と Windows / Linux SDK の成功を必要とする。

v1.9 の残る native kernel の公平性、GUI 拡充、履歴費用、アルゴリズム、v2.0 の統合条件は [計画](ROADMAP_v2.md) に従い継続する。本版をもって全工程の完了とはしない。
