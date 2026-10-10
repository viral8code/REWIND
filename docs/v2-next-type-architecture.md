# 追加構想：型の分割・生成・抽象化

状態：採用判断前。現在の仕様ではありません。版番号は未割当です。[選定表](v2-next-selection.md)の先に整理した34候補に加える8候補です。

目的は、大きな型や自動生成APIを保守しやすくすることです。C#のpartial等を参考にしますが、現行のstruct/record/enum/trait、module visibility、ownership/effectを出発点とします。class継承体系の導入とは別の判断です。

## TYPE-01：partialによる型定義の分割

**用途：** GUIの生成部分と手書きevent処理、DB recordとmapper、protocol型と検証処理を、同じ型の複数fileへ分ける。

初期候補は同じmodule内のpartial struct/recordです。fileが同じdirectoryにあるだけでは同一moduleとしません。現在のmodule/importモデルを確認し、参加するfragmentを明示的に列挙する方式から検討します。任意directory scanやfile順に依存する合成は避けます。

compilerはfragmentを収集してから一つの型へ合成し、その後に型・visibility・Share/Send・ownership・effectを検査します。全fragmentの型種別、generic parameterと制約、公開範囲を一致させます。field/method重複と、partialでない定義との衝突を関連位置付きで診断します。

field追加を許すと位置constructorの順序、ABI、signature hash、serializationへ影響します。初期はprimary fragmentだけにfieldを置き、補助fragmentにはmethod/関連定義を置く案を優先して比較します。複数fragmentのfieldを許すなら順序を宣言し、file読込み順へ依存させません。

**受入：** fragment順を変えても同じ意味・公開signatureになる。private memberへ同じ合成型からアクセスでき、別moduleの便乗を拒否する。generic、record Share、重複field、生成fileの欠落を検査する。

**判断：** 型そのものを分割する必要がなければ、既存impl/module分割と生成mapperで解決する。partial enum/traitは初期範囲に含めない。

## TYPE-02：生成コード向けhook

C#のpartial methodを参考に、生成側の処理へ利用者の検証・変換を差し込む案です。まず明示trait/default method/callbackで同じ用途を実現できるか比較します。

省略可能hookを採用するなら、実装がないときに引数式も評価されないのか、空の処理を呼ぶのかを固定します。副作用やowner消費が有無で変わる仕様を安易に導入しません。戻り値、effect、capture、関連型、必須hookと任意hookを区別します。

**受入：** 生成コード更新で手書きfileを上書きしない。hook欠落/重複/効果不足を診断する。通常traitで十分なら新構文を保留する。

## TYPE-03：宣言的なderiveとschema生成

DATA-01の明示schemaが成立した後、Serialize/Deserialize、field検証、DB mapping等の反復記述をcompilerまたは明示generatorで生成する案です。現行trait/関連型を再実装しません。

生成できる型、field rename、skip、default、NULL、versionを限定します。Secret、native resource、closure、借用fieldを自動serializeしません。generated signatureは通常のcheckerを通し、origin位置と生成理由をLSP/docへ出します。

初期generatorはschema入力から明示的にfileを生成する形も比較します。compiler pluginを無制限に実行するmacro体系は前提にしません。生成範囲とnode/work/depth予算を設けます。

**受入：** 手書きmapperと同じ結果・error分類。生成差分をreviewでき、原稿の順序だけで不要な差分が出ない。partialは任意の組合せ候補であり必須依存にはしない。

## TYPE-04：propertyと検査付きfield更新

公開fieldを書き換える代わりに、getter/setterや検査付き更新を呼出し側から統一的に扱う案です。private fieldと明示accessorで不足する例を先に作ります。

propertyを採用するなら、getterがpureか、計算量、setterの失敗型、mutable borrow、evaluation countをsignatureへ表します。compound assignmentやpatternがgetterを複数回呼ばない規則を決めます。debug watchから任意getterを実行しません。

**受入：** 検証失敗で旧stateを保持する。effectが隠れず、型検査で確認できる。property風の見た目だけでphysical操作を起こす既定にしない。

## TYPE-05：trait objectと動的dispatch

異なる実装を同じcollectionへ入れる、runtime選択した描画/codec strategyを扱う案です。enumとgenericが既存であることを踏まえ、型を列挙できない具体例を作ります。

object safety、関連型の固定、generic method、receiver borrow、ownership、Share/Send、drop/cleanup、callback effectを規定します。vtable/addressをartifactやtraceのidentityにしません。dynamic dispatchの費用をprofileで区別できるようにします。

**受入：** ownerを消費するmethodと借用methodを安全に呼べる。無効なobject化を拒否する。revert/GC後のobject寿命とSecret表示が正しい。

## TYPE-06：extensionとcoherence

既存型へ補助methodを足す案です。free functionで十分な場合は構文追加をしません。既存trait implの所有module制約と重複検査を確認します。

extensionはimportで明示し、名前衝突時に優先順位で黙って選ばない方針を候補にします。private fieldへの特権アクセス、外部moduleでのfield追加、native resourceの偽constructorを認めません。trait implとの解決規則を決めます。

**受入：** import変更で意図せず別methodを呼ばない。診断に候補と定義位置が出る。generic特殊化予算を維持する。

## TYPE-07：値parameterと固定長型

固定長vector、dimension、protocol packetの長さを型で検査するconst generic候補です。現行runtime shape検査と何が違うかを例で示します。

初期は非負整数の限定的な値parameterです。無制限const evaluationや任意の外部観測を許可しません。特殊化数、型深さ、型同士の等値性、artifact signatureを規定します。大配列をstackへ置く機能とは分けます。

**受入：** 長さ不一致をcompile時に拒否する例がある。型で確定しないdynamic shapeは既存APIで扱える。小さな値の違いでbinaryが無制限に増えない。

## TYPE-08：明示変換とmatch支援

enum、tuple、recordのpatternは既存です。record更新、条件付き分解、型間変換の記述を減らす案を、既存matchとfactoryで書いた例と比較します。

追加するならfieldの部分move、borrow、private、default、失敗型を保ちます。任意のconstructorやgetterをpatternから暗黙に呼びません。未網羅診断の意味を弱めず、既存pattern解析予算を維持します。

**受入：** ownerを含む値で二重利用を拒否する。新構文と旧構文のdesugaringが同じ効果・評価順・errorになる。

## 調査の順序

TYPE-01/02の分割方式と既存impl → TYPE-03の明示生成 → TYPE-04/06の具体例 → TYPE-05/07/08の独立設計。

TYPE-01とTYPE-03は比較的明確な利用場面があります。dynamic dispatchやconst genericはcompiler全体への影響が大きいため、便利機能の小さなpatchとして混ぜません。各案の採用・保留は[選定表](v2-next-selection.md)へ記録します。
