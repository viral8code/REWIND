# byte parser combinatorの実装記録

更新日：2026-10-11。状態：**source実装済み・実行検証省略**。

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

## 費用と境界

literalは期待byte列のprefixを順に検査する。offset計算から毎回Unicode先頭scanをする方式は使わない。合成したgrammarのbacktrack回数やnative byte操作は既存VM予算へ課金される。packrat cache、左再帰停止、無制限grammar再帰の保証は追加しない。

期待labelはcallerが渡す説明。入力dataをerrorへ自動埋込みしない。期待集合は最大32件で、全候補を完全に保持する仕様ではない。

## 確認用source

libraries/std/tests.rwにchoice成功、同位置の期待集合、byte位置、trailing input、cut、optional、repeatの最小・最大とzero progressを追加した。実行はユーザー方針により省略。

SDK/standaloneの標準module登録へ追加する。公開CI/CD、main統合、Release、版更新は行わない。
