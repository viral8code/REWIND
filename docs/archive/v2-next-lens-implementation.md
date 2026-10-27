# 型付きlensと検査付き編集

更新日：2026-10-12。対象：codex/develop。**source実装済み・実行検証省略**。

[VOPTIC-01/02/04/06/10](visions/31-bidirectional-data.md)の基本部分をstd.lensに実装した。Root / Value / Errorの型を保つpure callbackの合成であり、reflection、private fieldへの特権、compilerによるfield path生成を追加するものではない。

## 型と合成

Lens<R:Share,V:Share,E:Share>はprivateなgetterとResult付きsetterを持つ。createで純粋なSend+Share callbackを明示し、get、set、modifyで読む・更新する。setterは新しいRootを返し、元のRootを変更しない。複数fieldを組にしたValueを使えば、整合条件の検査をsetterへ集約できる。

compose(outer,inner)はRoot→中間値→Valueを合成する。getterでは順に一回ずつ読む。setterはouterを一回読み、innerの更新が成功した場合だけouterのsetterへ結果を渡す。内側の検査失敗は外側の更新を呼ばずに返す。Error型は合成する両側で一致させる。

callbackは通常のmodule visibilityに従う。privateな型の作成側が安全な編集能力だけをLensとして公開する使い方を許すが、利用側がLensを作ればprivate fieldを読めるという仕組みではない。値型・callbackの型から誤ったRoot/Valueを渡せない。

## 欠損するfocus

optionalはgetterがOption<Value>、setterがResult<Option<Root>,Error>のOptionalLensを作る。previewで有無を読む。setIfPresent / modifyIfPresentは欠損時にOk(None)を返し、setterやtransformを呼ばない。途中のobjectを自動生成しない。

composeOptionalは欠損を伝播し、asOptionalで全域Lensを組み込める。検査のためのgetter呼出しが重なる場合がある。callbackはpureであり、getterに外部作用や評価順依存の更新を置かない。欠損部分を作るupsertは今回のAPIに含めない。

## 型付き変更

planはLensとfocusのbefore/afterだけをChange<R,V,E>へ保持する。applyは指定された等値関数で現在のfocusとbeforeを照合し、違えばConflict、setterが拒否すればRejectedと元のErrorを返す。新しいRootは成功時だけ返す。

対象外のfieldは前提に含めない。別fieldが変わったRootへ同じfocus変更を適用できるが、domain上連動するfieldがある場合はValueを組にするかsetterで再検査する。getter/setterが何を読むかをlibraryがreflectionで推定するものではない。

before / afterで編集値を読み、inverseで交換する。元の状態へ戻るかはLensと等値関数の法則に依存する。任意の丸め・正規化setterで厳密な逆変換を約束しない。

Lens、OptionalLens、Changeはfunctionを含む。既存freezeが拒否するfunction値をdata snapshotや跨runtimeの変更packへ保存できるようにしたわけではない。必要ならbefore/after等のdataを別に保持する。実行checkpointの名前や外部操作は扱わない。

## 法則のサンプル確認

checkLawsは指定したrootと二つの値に対するget-put、put-get、put-putをLawReportで返す。途中のsetter失敗はErrorで返す。比較は利用者が渡すpureな等値関数を使う。

これは有限の一例を調べる関数で、全入力に対する証明・自動property生成ではない。正規化を伴うsetterでfalseとなることを隠さず、利用側が厳密なLens法則とは異なる契約を選べるようにする。

## 費用と確認source

library自身はcallbackの合成と小さいrecordを追加する。Root全体のcopyや構造共有の程度はsetter実装に従う。自動的にpath上だけcopyするnative lensを実装したとは扱わない。callback panicとVM budgetは通常のResultへ変換しない。

libraries/std/tests.rwへ3件追加した。入れ子更新、検査失敗、他fieldの保持、逆変更、衝突、欠損時のcallback非実行、optional合成、法則違反を扱う。実行は省略。利用例は[typed-lens.rw](../../examples/typed-lens.rw)。SDK moduleへ登録したが生成・公開は行っていない。
