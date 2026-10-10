# 次版構想：言語表現・型・変更履歴

状態：採用候補。現在の仕様ではありません。版番号は未割当です。[全体計画](ROADMAP_v2-next.md)のv2.1/2.2を優先し、以下は最小例と互換性を確認してから採否を決めます。

## 現行の土台

2.0にはgeneric、trait、関連型、default method、where、tuple、型alias、名前付きconstructor/variant、nested pattern、match式、Resultの`?`、closure capture、ownership/effect検査があります。「他の言語にあるから」という理由でこれらを再実装しません。

確認先は[言語章](book/chapters/02-language.md)、[所有権・状態章](book/chapters/03-state.md)、[言語reference](language-reference.md)、`src/v2.rs`と`src/v2/v05/`です。以下の便利機能も、実装開始時にはparser/checker/runtimeを確認し、すでに可能なら文書や例の追加に縮小します。

## LANG-01：呼出し側の名前とdefault引数

長いGUI/HTTP/数値設定で引数の順番を間違えにくくする案です。まずrecord型の設定を渡す方式で不足する例を作ります。名前付きconstructorがあることと、任意の関数の名前付き引数は分けて評価します。

採用するなら位置引数との混在、重複、未知名、generic推論、引数評価順、default評価時点を固定します。default式は暗黙に外部観測・mutable state捕捉をしない範囲から始めます。parameter名が呼出しAPIになるため、改名の互換性をsignature/SDKへ記録します。

**受入：** 間違えやすい既存呼出しを改善できる。move/borrowと評価順を変えず、複数の候補へ曖昧に解決しない。設定recordで十分なら新構文を保留する。

## LANG-02：異なるerror型の明示変換

現行`?`は失敗型を合わせる必要があります。入力、DB、通信のerrorをapplication enumへまとめる反復記述を減らす案です。先に既存`std.result.mapError`を使う例を整備します。

追加候補は明示変換traitに基づく伝播です。任意の型同士の暗黙変換やString化は導入しません。変換の一意性、effect、元cause、Secret redaction、送信状態の保持を検査します。ownershipを持つerrorも、Share専用helperでは処理できないことを考慮します。

**受入：** Error enumへ分類とcauseを残せる。曖昧変換を拒否し、元のerror位置を診断できる。予算超過/panicを通常Resultへ自動捕捉しない。

## LANG-03：型付き不透明ID・単位

user ID、row ID、window ID、byte offsetを同じInt/Stringのaliasだけで区別しにくい場合に、private record/structのwrapperを使う設計を整備します。新しいnominal alias構文は既存wrapperの費用・記述量が問題と確認できたときだけ検討します。

IDの生成、比較、serialization、DB mapping、Ord/Share、公開factoryを揃えます。秒/ms/ns、byte/scalar/grapheme、行位置と行identityを明示するSDK候補を洗い出します。すべての既存引数を一度に置換する破壊的変更にはしません。

**受入：** 異なるID/単位の誤用を型検査で拒否する例がある。内部表現と変換費用が説明できる。legacy APIとの変換を明示できる。

## LANG-04：generic collection helperの範囲

`Map<K,V>`は既存ですが、`std.map`の便利関数にはStringキー用のものがあります。`std.collections`、`sequence`、Iteratorも既存です。まず利用者定義Ordキー、mutable値、affine値で使えない具体例を整理します。

候補は型契約を満たすgeneric helper、Resultを返すmap/filter/fold、bounded collectです。snapshot Iteratorの保証を保ちます。borrowed iteratorを検討する場合は、checkpoint、owner変更、capture、taskへの逃避を別の契約として設計し、既存iterの意味を変更しません。

**受入：** key型ごとの順序・比較費用を記載する。affine値を複製しない。短絡、callback failure、容量超過で何が消費済みかを示す。

## LANG-05：消費するResult/Optionの操作

Share制約付きのmap/helperは既存です。connectionやnative ownerを含むResultで、毎回`match move`を書く負担を減らす案を調べます。

まず「消費する入力」「失敗時のowner」「callback capture」「再利用不可」を表すsignatureが現行genericで成立するか確認します。ownerをcloneする便利関数にはしません。fallible callbackで取り出した資源を失わない設計が必要です。

**受入：** DB接続の生成結果を安全に次操作へ渡す最小例。二重利用・borrow逃避を拒否。失敗/取消/早期returnでcleanupが一度だけ成立する。

## LANG-06：複数errorを集める検証

最初のErrで停止する`?`と、form/設定の複数誤りを一度に返す検証を区別します。recordやenumでpath、code、関連fieldを持つ結果を設計し、検証自体はpureにします。

件数上限と省略情報を持たせます。成功値とerrorが同時に存在するとき、部分値を保存操作へ流用できるかを型で分けます。offsetの単位を固定し、機密な入力値をmessageへ埋め込まない設計にします。

**受入：** nested JSON、設定、formで同じ検証結果をCLI/GUIへ表示できる。無制限error収集でmemoryを消費しない。検証失敗で外部保存しない。

## STATE-01：checkpoint保持方針のhelper

commit/dropとGCは既存です。画面のUndo履歴を利用者が管理しやすくする、明示容量付きhistory helperを検討します。VMが任意のcheckpointを自動削除する仕様にはしません。

利用者がhelperに所有を委ねた履歴だけに最大件数・label生成・redo取消・明示clearを適用します。begin、helper外のcheckpoint、branch、終了済scopeを区別します。任意labelを動的に扱えるかは既存構文/VMを確認してからAPIを決めます。

**受入：** Undo/Redoと新編集時の分岐整理ができる。保持対象と破棄対象を説明できる。publish済みの作用を取り消したふりをしない。新構文なしで成立するならlibrary/例を優先する。

## STATE-02：publish結果の型とアプリケーションの判断

公開済み・未送信・結果不明等の既存情報を、アプリケーションが判断しやすい形で使う案です。新しい情報を取得できると仮定せず、現在のdiagnosticとAPIを調べます。

候補は操作ごとの結果一覧、cause、再送の可否を利用者が判断するための型付きadapterです。複数File/出力/HTTP/DBに跨るatomic commitは約束しません。機密値ではなく操作IDとcodeを使います。

**受入：** 部分適用と結果不明の例で、UIが「全失敗」と誤表示しない。自動retryしない。成功済み操作をrevert後に再送しない。

## LANG-07：長期保守に必要なAPI変化の通知

deprecated APIの通知、移行先、適用languageをsource/LSP/docへ一貫して出す案です。SDK signature、公開field、effects、generic bounds、error codeの差分を互換性別に分類します。

警告をruntime失敗として扱わず、機械可読なcodeと抑制範囲を設計します。formatterや移行toolがeffect/ownershipを変える書換えを自動適用しません。候補はまず報告のみです。

**受入：** 一つのAPI変更からCLI/LSP/docが同じ移行理由を示す。古いartifactの実行可否とsourceの警告を区別する。

## 採否の順序

LANG-06とLANG-03のlibrary/例 → LANG-04/05の型成立性 → LANG-01/02の構文・互換性 → STATE-01/02 → LANG-07。

先に使う場面を二つ以上作り、既存APIで解決できない差分を示します。overload、例外体系の全面置換、class継承、任意reflection、macro体系は、この文書から自動的に採用しません。必要ならcompiler費用とownership/effectの相互作用を含む独立案を作ります。
