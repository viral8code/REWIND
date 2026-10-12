# text版を保持する位置型

更新日：2026-10-12。対象：codex/develop。**source実装済み・実行検証省略**。

[VTEXT-01](visions/45-text-codecs.md)のうち、UTF-8 byte、Unicode scalar、extended graphemeの位置をstd.textIndexへ実装した。DisplayColumn、正規化・編集の位置mappingは含めない。既存std.text/std.bytes/std.unicodeのInt index APIは変更しない。

## 値と入口

SourceはString、UTF-8 Bytes、scalar/graphemeの境界表をprivateに保持するimmutable record。create(text)は最大1 MiBかつ65,535 scalarを受け、Result<Source,IndexError>を返す。端の境界を含む表は各65,536件まで。grapheme分割には既存stdUnicodeOffsetsを使い、Unicode ruleを別実装しない。

ByteOffset、ScalarIndex、GraphemeIndexはそれぞれSourceを保持するprivate fieldのrecord。異なる単位を関数へ渡すと型が一致しない。byteAt/scalarAt/graphemeAtで範囲検査し、byteValue/scalarValue/graphemeValueで数値を明示的に取り出す。各位置のsourceはbyteSource/scalarSource/graphemeSourceで読める。text/bytesと各lengthでSourceを参照する。

位置は0始まり。終端位置を許すので空textにも位置0がある。ByteOffsetはscalar内部の任意byteを表せるが、byteToScalarやbyteToGraphemeは正しい境界以外を拒否する。scalarToByte/graphemeToByteは必ず境界を返す。scalarとgraphemeの変換はbyte位置を経由して明示する。

## 原文と範囲

位置を作った後に元の変数を書き換えても、位置は以前の原文を保持する。別textへ暗黙に付け替えない。sliceBytes、sliceScalars、sliceGraphemesは型の一致した始端と終端を受け、半開区間を返す。

範囲の両端は原文のString内容が同じことを照合する。異なるSourceでも原文が完全に同じなら利用できる。VM固有IDやeditor revisionを生成したわけではなく、内容の異なるtextへ旧位置を使う場合はTextMismatchとなる。範囲が逆ならRange。書式・正規化・改行は変えない。

sliceBytesはUTF-8の途中も切れるraw Bytesを返す。scalar/graphemeのsliceは境界検査済みのUTF-8からStringを返す。結合文字の前やCRLFの間はscalar境界でもgrapheme境界ではない。

## 費用と寿命

createはUTF-8と二つの境界表を一度構築する。byte→scalar/graphemeの変換は境界表を二分探索し、逆変換はList lookup。各List操作の既存storage費用は別に掛かる。変換のたびに先頭から文字を数え直さない。

範囲の原文照合は最悪text byte長に比例する。sliceは結果byte列とStringを生成する。全文を保持した位置を一個残すと、原文と境界表も保持される。detachしたい場合は必要なsliceだけを取り出し、Source/位置を保持するownerを解放する。dataの保持であり、外部fileの版や権限を所有しない。

Resultの失敗はIndexError(code,offset)。ByteRange、ScalarRange、GraphemeRangeのoffsetはそれぞれ指定した単位、Boundary/TextMismatch/Rangeはbyte位置。codec/Unicodeの失敗codeは元のStdErrorから引き継ぐ。fatal VM budgetは通常のResultへ変換しない。

## 確認source

libraries/std/tests.rwへ2件追加した。emoji内部、結合文字、CRLF、日本語、終端、空入力、位置を作った後の原文変更、別原文の拒否、同内容Source、逆向き範囲を扱う。実行は省略。型の取り違えに対するcompile-fail確認、最大入力とallocation/GC測定は未実行。

利用例は[examples/text-positions.rw](../examples/text-positions.rw)。expected出力は期待値で実測結果ではない。SDK module registryへ登録したが、SDK/API生成・公開はしていない。
