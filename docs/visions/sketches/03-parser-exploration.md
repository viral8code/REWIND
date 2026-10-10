# Sketch：曖昧な入力を探索するparser

## 使いたい体験

いくつかの解釈がある入力を、branchで試して比較する。最初に成功したもの、最も多く入力を消費したもの、scoreが良いもの等をpolicyで選ぶ。

失敗した試行の入力位置・理由は保持するが、cursorや構築途中のsyntax treeは試行外へ漏らさない。parserの探索と、ユーザーへ示す診断を同じ履歴から作りたい。

## 組み合わせる構想

- 巻き戻せるInputTapeとcursor snapshot。
- choice、探索strategy、候補の説明tree。
- checkpoint外の診断・統計。
- typedなsyntax値とsource位置。
- 共通prefixのmemoization。

## 擬似コード

~~~text
hyper var diagnostics = ParseDiagnostics();
let origin = snapshot.capture(&cursor);

let candidates = search.from(origin)
    .choose([parseAsDeclaration, parseAsExpression])
    .onFailure(|problem| diagnostics.record(problem))
    .evaluate();

let accepted = candidates.select(LongestValidPrefix)?;
cursor.replaceFrom(accepted.cursorSnapshot);
return accepted.syntax;
~~~

このcodeは現在のAPIではない。関数のcallee frameを保存して後からrevertする設計ではなく、cursor等のdata snapshotを操作する。

## sourceがstreamの場合

まだ取得していないphysical入力をbranchごとに読み直さない。取得済みのbufferをInputTapeとして共有し、足りない部分は明示的な取得段階で補う。

prefixの保持範囲と未取得範囲を分ける。candidateがcursorを保持する間は、その範囲の入力もrootとして残す。

## 診断の作り方

失敗した各branchの位置、期待token、選択経路を比較する。最も遠くまで進んだ失敗だけを出す方式と、曖昧な解釈をまとめて出す方式を選ぶ。

checkpoint外の診断は、別の入力へ切替えたときにはsession identityで区別する。古い試行の診断を新しい入力へ混ぜない。

## 計算の共有

同じ位置で同じruleを試す計算は、入力・rule/code version・modeをkeyにする。syntax treeとcursor結果をimmutableな値として共有する。

失敗結果もpureな条件に依存する範囲でcacheする。予算切れを「このruleではparseできない」という永続的な失敗へ変換しない。

## もう少し広げるなら

error recovery案を候補として返し、editorが「このtokenを補うとこう読める」をpreviewする。修正の採用は利用者が選ぶ。

同じengineをschema inference、protocol検査、text commandの解釈にも使う。入力を決め打ちする前に、複数の可能性を調べられる道具にしたい。
