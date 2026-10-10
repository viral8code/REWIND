# 式・pattern・日常の書き方

新syntaxの実装依頼ではなく、短いcodeでも型・効果・所有権が読みやすくなる方向の自由構想です。既存syntaxで可能な部分は、後で差分を確認します。

## VEXPR-01：Optionに沿うmember chain

値がSomeなら次のfieldやoperationへ進み、Noneならそのまま返す。深いOptionを毎回nested matchへ展開せず、型を保ったchainを書きたい。

ownerを取り出すchainと、借用して読むchainを区別する。途中で式を二回評価する変換にはしない。

## VEXPR-02：lazyなfallback式

値がないときだけ、代わりの計算を行う演算を使いたい。fallbackが重い処理やowner移動を含む場合も、必要なときだけ評価する。

Optionの不在とResultの失敗は別に扱う。errorを握りつぶすdefaultへ暗黙変換しない書き方が欲しい。

## VEXPR-03：typed pipelineとplaceholder

前の結果を次の関数の明示位置へ渡す。変換の順序を左から読め、途中の型をeditorで確認したい。

任意のtext置換ではなく、引数評価順とeffectを保つ構文を考える。複数回使うplaceholderでownerを複製しない。

## VEXPR-04：partial application

関数の一部の引数を固定して、残りを受け取る関数値を作る。capture value/move/borrowと、一度だけ呼べるかを表示したい。

引数の固定に外部観測があるなら、その評価時点を明示する。関数を作る操作と、後で実行する操作の効果を分けたい。

## VEXPR-05：bindingのdestructuring

tuple、record、variantの構造をbindingで分解する。取り出すfield、rename、捨てる値を簡潔に書きたい。

refutableなpatternは失敗経路を要求し、部分moveした値を再利用しない。private fieldの公開範囲も保つ。

## VEXPR-06：patternで列挙を絞るloop

列挙中の特定variantだけを処理する書き方を考える。失敗したpatternをskipするかerrorにするかを明示したい。

callbackのfilterと組み合わせ、snapshot cursorやowner消費の意味を保持する。短い構文でも何が捨てられるかを知りたい。

## VEXPR-07：label付きtuple

小さな戻り値に名前を付け、fieldの意味を示す。新しい公開recordを作るほどでない結果も、位置だけに頼らず読めるようにしたい。

labelが型identityやAPI互換へ影響するかを決める。labelのあるtupleと普通のtupleの変換も明示したい。

## VEXPR-08：rest patternの用途

必要なfieldだけ分解して残りを扱う。新しいfieldが増えたときのmatch互換と、知らないfieldを捨てる操作を分けたい。

残りのownerを保持する場合と完全に破棄する場合を区別する。field追加で想定外のcleanupを省略しない表現が欲しい。

## VEXPR-09：pureなactive pattern

利用者が定義した分解関数をpatternとして使う。URLの構造、検証済みtext、数値の分類を、通常のmatchへ接続したい。

patternの計算量とpure契約を表示する。試すたびにnetworkやmutable getterが呼ばれるような隠れた効果にはしない。

## VEXPR-10：型付きoperator定義

利用者のmatrix、単位、symbolic式へoperatorを定義する。演算の優先順位と戻り値・effectを明示し、関数呼出しとの対応を確認したい。

記号を増やすだけで読みづらくならないように、importと名前衝突を扱う。checkedな失敗を戻り値の型で保つ。

## VEXPR-11：expression blockの結果

数行のbinding・検査を、一つの値を返すblockとして書く。途中のreturnとblockの結果を区別し、ownershipを移す地点が分かる形にしたい。

match armやinitializerの読みやすさを揃える。最後の式だけで意図しない外部確定が起きない契約を保つ。

## VEXPR-12：短いerror context付与

失敗を伝播する地点へ、operation名やtypedなcontextを添える。長いmatchを書かずにcauseを保ち、callerが分類できる結果にしたい。

contextへ入力値を何でも埋め込まず、codeや位置を中心にする。異なるerror型の変換と、説明を添える操作を区別する。
