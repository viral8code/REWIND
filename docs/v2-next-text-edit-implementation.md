# 文字編集と版を跨ぐ位置mapping

更新日：2026-10-12。対象：codex/develop。**source実装済み・実行検証省略**。

[VTEXT-01](visions/45-text-codecs.md)のedit mappingと、VTEXT-06の原文を維持する部分更新をstd.textEditへ追加した。std.textIndexの原文付き位置型を使い、一つの置換の前後をimmutableなEditとして持つ。

## 編集を作る

replaceはScalarIndexの半開区間とStringを受ける。両端の原文が同じで順序が正しいこと、編集後のscalar数が65,535以内であることを確認する。replaceBytesも提供するが、UTF-8 scalarの途中を切る位置はScalarBoundaryで拒否する。replaceGraphemesはgrapheme境界から同じ処理へ渡す。

置換範囲の外は元のStringをそのまま繋ぎ、正規化や改行変換を行わない。既存Sourceの上限とUnicode分割規則を引き継ぐ。置換内容が元と同一なら同じSourceを保持して境界表の再構築を省略する。

source / resultは前後のSource、start / endは旧版のbyte範囲、insertedBytesは挿入byte数、isIdentityは内容が不変かを返す。Editは前後両方の原文と境界表を保持する。元のSourceや位置を破壊的に更新しない。

## 位置の変換

mapByte、mapScalar、mapGraphemeはEditと旧位置、Policyを受ける。原文の内容がEdit.sourceと異なる場合はTextMismatch。元の位置型を単なるIntとして新しいtextへ当てはめない。

- 編集範囲より前は同じbyte位置、後ろは置換byte数の差だけ移動する。
- 非空範囲の始端・終端は、それぞれ新しい範囲の始端・終端へ正確に対応する。
- 置換範囲の内部は元の位置が失われる。ExactはDeleted、Before/Afterは新しい範囲の始端/終端へ寄せる。
- 長さ0の挿入位置は前後二通りの対応を持つ。ExactはAmbiguousInsertion、Before/Afterでどちら側へ置くか明示する。
- 内容が変わらないEditはすべての位置をそのまま写す。

mapScalarとmapGraphemeは新しい版で境界を再検査する。たとえば文字の後へ結合記号を挿入すると、旧いgrapheme境界がなくなる場合がある。黙って別graphemeへ吸着せずGraphemeBoundaryを返す。byte位置が対応しても、その場所の意味やtoken identityが同一という証明にはしない。

inverseは前後のSourceと範囲を交換したEditを返す。旧原文へ戻れる一方、削除された内部位置の多対一mappingが自動的に可逆になるわけではない。実fileの変更を巻き戻す機能ではない。

## 費用と限界

一回の編集は必要なsubstringと新しいStringを作り、新版の境界表を構築する。全文長に比例するcopy/scanがあり、rope・piece table・増分grapheme解析は今回含めない。多数の一文字編集が一定時間になると宣伝しない。

位置変換の幾何計算は定数回の整数演算。原文照合は最悪text長、scalar/grapheme変換は境界表の検索費用を持つ。内容が同じ別Sourceも受け付けるため、editor固有revision IDを発行するものではない。

失敗はEditError(code,offset)。原文・範囲・mappingの失敗は旧版のbyte位置、境界の再検査は新版のbyte位置を返す。native codecや新版構築の失敗位置はその処理から引き継ぐ。VMのfatal budget失敗はResultへ変換しない。

## 確認source

libraries/std/tests.rwへ3件追加した。日本語→emoji置換、削除位置のpolicy、終端とscalar位置の移動、逆編集、挿入の曖昧さ、結合文字によるgrapheme境界消失、無変更、原文違い、逆範囲、UTF-8内部の拒否、全削除を扱う。実行は省略。

利用例は[text-edit-mapping.rw](../examples/text-edit-mapping.rw)。module registryへ登録したが、SDK/API生成・公開は行っていない。
