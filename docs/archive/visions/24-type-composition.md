# 型の合成・抽象化・polymorphism

generic/trait/関連型がある現在のREWINDを前提に、その表現範囲を広げる自由構想です。複雑な型を増やすことより、APIの意図を失わず共通化できる用途を考えます。

## VTYPE-01：row-polymorphic record

必要fieldを持つrecordなら、追加fieldが違っても同じhelperへ渡す。読取りだけのhelperを、似たrecordごとに書き直さずに済ませたい。

fieldの追加・更新と、opaque/privateな型の境界を区別する。構造が同じことを、同じdomain identityという意味にはしない。

## VTYPE-02：open/closed variantの選択

plugin等で増えるvariantと、網羅性を厳密に検査する閉じたenumを使い分ける。APIの拡張性を型宣言へ表したい。

未知variantを保持・転送する場合と拒否する場合を明示する。すべてのmatchへ無条件にwildcardを要求する設計にはしない。

## VTYPE-03：callbackのeffect polymorphism

callbackが要求するeffectをhelperのsignatureへ引き継ぐ。pure版・output版等を同じ処理構造で書けると嬉しい。

どんなcallbackでも許す曖昧な関数にはせず、許可されるeffect集合を表す。captureやSend/Shareの条件とも組み合わせる。

## VTYPE-04：opaqueな戻り値型

具体的な実装型を公開せず、利用者が必要とするtraitを満たす値を返す。内部のIterator chainやstrategyを変更しても、公開APIの詳細を増やさずに済ませたい。

staticな実装を隠す方式とdynamic objectを返す方式を区別する。所有権や効果も隠れた型の一部として残す。

## VTYPE-05：traitの関連value

型に関連したdimension、上限、既定policy等を契約として持つ。関連型だけでなく、値で表す性質をgenericなcodeから使いたい。

compile-timeで使える値とruntimeで問い合わせる値を分ける。型の識別と外部観測を結び付けない。

## VTYPE-06：限定したtype function

型からfield型、element型、結果型等を導く操作を用意する。schema helperや変換APIの戻り型を、手作業の重複宣言なしに表したい。

型計算をeditorで展開して確認する。任意のprogram実行と同じ無制限なcompile-time処理にはしない。

## VTYPE-07：union/intersectionの用途

「AかB」「複数のcontractを満たす」を型として組み合わせる。enum constructorを毎回作る方式と比較し、小さなAPIの共通化へ使いたい。

mutable fieldやmethodの衝突を扱う。unionの値をどちらとして使うかは、patternや明示変換で確認する。

## VTYPE-08：existential package

隠した型と、その型へ操作する関数を一緒に渡す。callerが具体型を知らなくても、整合する操作を使えるpackageにしたい。

serializerとdata、solverと内部state等を同じ組で保持する。異なるpackageの内部型を混ぜないようにする。

## VTYPE-09：GADT的なvariant

variantごとに結果型や型の関係を表す。typedな式木やprotocol stateをmatchすると、分岐内で使える型情報が増える仕組みを考えたい。

構築できた時点で分かる条件を、消去せずに分解へ引き継ぐ。証明を持つ値の構想とも接続する。

## VTYPE-10：varianceを説明するgeneric型

型parameterを読取りに使うか、書込みに使うか、両方かを宣言・推論する。genericなviewとownerの変換がなぜ安全・不正かを説明したい。

見た目が似たcontainerを同じmutable型として扱わない。型変換の費用やcopyの有無も明示する。

## VTYPE-11：higher-rankな借用callback

一回限りの借用期間ごとに使えるcallbackをgenericに受け取る。calleeが一時viewを渡しても、その参照をcallbackから長期保存できない契約が欲しい。

scopeごとの処理、iterator view、native loanに使う。固定した一つの長寿命借用とは分けて表す。

## VTYPE-12：領域が作るgenerative type

arena、runtime、snapshot session等の生成ごとに、混用できない型identityを作る。別runtimeのhandleを同じ数値だからと渡す誤用を防ぎたい。

実行時identity検査と型による区分を組み合わせる。外部へ保存するdataのschema identityとは別のものとして扱う。
