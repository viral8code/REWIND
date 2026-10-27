# byte parser combinatorの実装記録

更新日：2026-10-12。状態：**source実装済み・実行検証省略**。

[parser構想](visions/46-parser-construction.md)のVPARSE-01/02/03から、有限入力の合成、期待集合、cutをstd.parserとして実装する。grammar generator、左再帰、stream追加待ち、incremental parse、parse forest、error recoveryは含めない。

## 型と入力

Parser<T:Share>はprivateなpure Send+Share functionを持つrecord。CursorはprivateなBytesとbyte offset。Parsed<T>はvalueとnext cursor、ParseErrorはcode、offset、Frozen<List<String>>のexpected、committedを持つ。

入力は最大1 MiBのBytes。parseは部分成功を返し、completeは未消費byteが残るとTrailingInputを返す。positionとremainingもbyte単位。Unicode scalarや表示columnに読み替えない。Stringを扱う場合は明示してBytesへencodeし、必要な箇所でdecodeする。

Cursorは通常のimmutable data。backtrackは元cursorを次のparserへ渡すだけであり、VM全体や物理入力をrevertしない。Parser本体はfunction値を持つため、data snapshotの保存対象としては扱わない。

## 合成

- literal：指定Bytesの完全一致。最初の相違byteまたは不足位置で失敗する。
- satisfy：pure predicateを満たす1 byteを読む。
- end：入力末尾だけで成功する。pure(value)とfail(description)も提供する。
- map：成功値だけをpure callbackで変換する。
- pair：順番に二つを読み、Tupleの値を返す。
- orElse：通常失敗なら元cursorから第二候補を試す。遠くまで進んだ失敗を選び、同位置では期待labelを重複除去して最大32件保持する。
- cut：包んだparserの失敗をcommittedにする。orElse/optional/repeatはこれを通常不一致として飲み込まない。
- optional：通常不一致をNoneへ変え、元cursorを保持する。
- repeat：0 <= minimum <= maximum <= 65,536。maximum回で追加probeをせず終了し、成功結果をFrozen<List<T>>で返す。消費なしの成功はNonProgressで拒否する。

cutは「このgrammarを選んだ」という意味であり、言語のcommit/publishとは無関係。repeatの最大数はgrammarとしての上限で、VMのinstruction/native/allocation budgetは別に働く。VMのfatal budget failureやcallback panicはParseErrorへ変換しない。

T:Shareに加えてrepeatの結果は既存freezeに従う。関数等のfreezeできない値をrepeatの収集結果に使うと、既存の型・実行時制約に従って拒否される。任意Share値がserialize可能になるわけではない。

## 先読み・値検査・原文範囲（2026-10-12）

lookAheadは成功値を返してcursorを元に戻す。notFollowedByは通常不一致なら消費せず成功し、成功した場合は呼出位置でMismatchとする。どちらもcut付き失敗は伝播する。消費しない成功をrepeat/chainへ渡す場合もprogress検査を通す。

verifyは解析値をpure predicateへ渡し、falseなら読み終えたbyte位置でMismatchを返す。andThenは成功値に応じて次のParserを選び、同じ入力の続きから実行する。利用者へCursorのconstructorを公開せず、callbackから別入力のcursorを差し込まない。callbackの再帰やpanicを自動的に安全化する機能ではない。

spannedは値とSpanを返す。Spanはprivateな原文Bytesと半開区間を持ち、spanStart/spanEnd/spanBytesで参照する。部分parserの開始位置を保持するため、先頭以外にあるtokenの原文も取得できる。元のBytes変数を変更しても意味は変わらない。Spanを一個保持すると原文全体も保持する。必要なbyteだけを残したい場合はspanBytesの結果を取り出してSpanを解放する。

原文範囲の保持はVPARSE-07の前提となる部分であり、lossless tree、commentの付け替え、増分parseを実装した意味ではない。

## 演算子の結合（2026-10-12）

binary(token,description,combine)はpure Send+Shareな二項関数を持つBinaryOperator<T>のParserを作る。chainLeft(term,operator,maximum)とchainRight(...)はterm (operator term)*を解析し、左結合／右結合で値を組み立てる。factoryは0〜65,535のmaximumを検査する。

左結合は累積値だけを保持し、右結合はoperand/operatorを一時Listへ置いて末尾からloopで評価する。右結合の長さに比例した関数再帰を避ける。maximumは各chainの呼出し単位で、grammar全体の総演算子数の制限ではない。右側operandと合わせてもcursorが進まなければNonProgress。maximum到達後はprobeしないので、追加operatorが残る場合はcompleteがTrailingInputとする。

operatorの通常不一致はchainの終わりとなる。operatorが成功した後のterm失敗はそのまま失敗とし、欠けたoperandを黙って読み飛ばさない。cut失敗も伝播する。callbackのpanic、overflow、VM budget failureをgrammar不一致へ変えない。

掛算のchainをtermとして足算のchainへ渡すと優先順位を表せる。[演算式の例](../../examples/parser-expression.rw)を参照。VPARSE-05の結合・優先順位の基本部分であり、operator tableの自動生成、prefix/postfix、non-associativeの宣言、曖昧なtoken prefixの自動解決は含めない。同じprefixのoperatorはliteralのchoice順を利用者が明示する。

## 費用と境界

literalは期待byte列のprefixを順に検査する。offset計算から毎回Unicode先頭scanをする方式は使わない。合成したgrammarのbacktrack回数やnative byte操作は既存VM予算へ課金される。packrat cache、左再帰停止、無制限grammar再帰の保証は追加しない。

期待labelはcallerが渡す説明。入力dataをerrorへ自動埋込みしない。期待集合は最大32件で、全候補を完全に保持する仕様ではない。

## 確認用source

libraries/std/tests.rwにchoice成功、同位置の期待集合、byte位置、trailing input、cut、optional、repeatの最小・最大とzero progressを追加した。実行はユーザー方針により省略。

追加拡張に5 test sourceを用意した。先読みのcursor、cut、原文保持、条件検査の失敗位置、値依存のgrammar、左/右結合、優先順位、欠落operand、上限とzero progressを扱う。実行は省略。

SDK/standaloneの標準module登録へ追加する。公開CI/CD、main統合、Release、版更新は行わない。
