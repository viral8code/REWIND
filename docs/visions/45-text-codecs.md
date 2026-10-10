# text・bytes・encodingの境界

既存文字列処理やcodecの発展案。byte列、Unicode text、表示上の文字を区別し、変換と位置対応をlibrary全体で共有したい。

## VTEXT-01：位置の単位を持つindex

ByteOffset、ScalarIndex、GraphemeIndex、DisplayColumn等を別の値型にする。parser、editor、検索結果の位置を誤って混ぜずに済む。

変換には対象textの版を持たせる。旧textのoffsetを新textへ使う場合は、edit mappingを明示的に通す。

## VTEXT-02：正規化の位置mapping

Unicode normalizationが結果だけでなく、元の範囲との対応も返す。検索用の正規形から利用者が入力した位置へ戻したい。

一対一でない対応はrangeや曖昧なmappingとして表す。正規化で見た目が同じになったことを、元bytesの同一性へ読み替えない。

## VTEXT-03：途中状態を持つdecoder

chunk間に分かれたmulti-byte文字をdecoder stateへ保持する。完了、追加入力待ち、不正sequenceを別結果にする。

decoder stateをdata snapshotにしてfixtureの分岐に使える。実fileやsocketの読取り位置は別ownerが管理する。

## VTEXT-04：losslessな不正text表現

不正bytesを置換して捨てる方式に加え、元bytesと場所を保持する表現を選ぶ。log、移行tool、editorで修正前のdataも扱いたい。

正常textへ変換する時のpolicyを明示する。replacement文字から元bytesが一意に復元できるとは扱わない。

## VTEXT-05：encodingを決める根拠

BOM、宣言、設定、推定等の根拠をEncodingDecisionへ持たせる。どの理由でこのencodingを選んだかを読む。

推定の確信度と検証済みのencodingを分ける。利用者が選び直すと同じraw bytesから再decodeできるmodelを考える。

## VTEXT-06：行末と原文の保持

CRLF、LF、混在した行末をparse後も保持し、編集した部分だけ変更する。formattingと原文維持を用途で使い分ける。

保存policyで統一する場合は差分をpreviewする。textの論理内容とfileのbyte内容のhashを別々に扱う。

## VTEXT-07：ropeとpiece tableの共通view

大きいtextを分割storageのまま検索、slice、parseするinterface。全文を毎回連続Stringへcopyせず使いたい。

viewはtext版とownerを持つ。小さいsliceが巨大な原文全体を保持する場合、detach copyを明示的に選べる。

## VTEXT-08：locale版付きのcollation key

sort用keyへlocale、規則、library版を付ける。保存したindexが別環境でも同じ順序になる条件を示したい。

元textの等値とcollation上の同順位を分ける。規則変更時にはindexの再構築案を返す。

## VTEXT-09：型付きのescape context

HTML text、attribute、URL component、SQL parameter等の用途を型で分ける。ある用途のescape済みStringを別contextへそのまま渡さない。

生成するdocumentの構造と連携する。SQL parameter binding等、escapeではない操作を同じ関数へ押し込まない。

## VTEXT-10：構造を持つformat結果

補間結果を単なるStringだけでなく、literal、値、単位、spanの列として得る。CLI、GUI、reportで同じ内容を表示したい。

Secretや数値精度はformatterのcontractに従う。formatの既定表示を、wire形式や署名対象の形式へ流用しない。

## VTEXT-11：text boundaryのquery

単語、文、grapheme、identifier等の境界を、選んだ規則で列挙する。editor操作と検索で同じboundary libraryを使う。

言語・用途に依存する境界を一つの普遍的規則としない。利用者がdomain-specificな規則を追加できると便利。

## VTEXT-12：連鎖codecのplan

decode、normalize、parse等の段階をplanとして並べ、各段階のerrorと位置変換を合成する。

どの段階が情報を失い、どの段階が可逆かを表示する。原文を保持するpolicyとmemory budgetも同じplanへ含めたい。
