# Sketch：domain言語を育てるworkspace

## 使いたい体験

rule、schema、query、数式等の小さな言語を作り、入力・型検査・展開・実行・説明を一つのworkspaceで扱う。利用者がDSLのerrorをREWINDの生成codeまで追わなくて済むようにしたい。

入力言語と生成codeの関係を保ち、生成部分と手書き部分を別fragmentとして管理する体験を考える。

## 組み合わせる構想

- parser探索、typed quotation、hygienic macro。
- GADT的な式treeと限定したtype function。
- module signature、partial fragment、生成hook。
- origin graphとstructured Diagnostic。
- pure compilation cacheと版付き入力。
- artifactのsignatureとlink plan。

## 擬似コード

~~~text
let document = DomainSource.load(selectedInput);
let syntax = parser.parse(document)?;
let typed = checker.check(syntax, domainSchema)?;
let lowered = lowerToTypedRewind(typed, preserving = OriginGraph);

let fragments = generator.emit(
    primary = domainModel,
    generated = lowered,
    hooks = DeclaredExtensionPoints);

let module = composeFragments(fragments, userImplementations)?;
let report = validateModule(
    module,
    signature = DomainRuleSet,
    effects = PureModelOnly,
    budget = CompileWorkBudget);

workspace.show(document, typed, lowered, report);
~~~

構文とAPIは未実装の擬似例。compiler pluginの導入や、任意codeのbuild時実行を既定事項にはしない。

## originをどう残すか

生成nodeに、元のspan、適用したrule、生成器版を付ける。DSLの一つのfieldが複数methodへ展開された場合も対応を辿れる。

runtime errorでは利用者向けspanと生成codeの位置を両方持つ。説明の詳細を開くまでは、利用者が書いた入力の位置を優先して表示したい。

## 展開の安全な範囲

型付きASTの合成は通常のcheckerを通す。生成したprivateアクセス、所有権の移送、effect等を、生成器だからという理由で免除しない。

展開のnode数、再帰、型計算、特殊化にbudgetを設ける。pureな生成段階で外部fileやnetworkが必要なら、明示的な入力やcapabilityを要求する。

## 手書きの差込み

generatorはhookのsignatureを公開し、手書きfragmentが実装を提供する。必須hookと既定実装のあるhookを区別する。

生成器版を上げてhookの型やeffectが変わったら、差分を表示する。fileが同じdirectoryにあるだけで合成対象へ追加する方式にはしない。

## 版付きの評価

ruleの入力schema、生成器、実装、依存moduleを同じ評価manifestへ残す。旧ruleと新ruleを同じdata snapshotに適用し、結果と費用を比較できる。

比較で使った入力には公開viewを選べる。schema変更で対応が失われた場合はmigration案を要求し、古いtraceを新しいsymbolへ黙って結び付けない。

## もう少し広げるなら

visual rule editor、textとの往復、条件の未到達説明、反例生成、定数foldの閲覧を組み合わせる。REWIND自体の仕組みを、別の小さな言語を作る道具として公開したい。
