# 所有権・領域・値の費用

現行のownership、borrow、Share/Send、GCを置き換えるという意味ではありません。それらを利用者がより細かく表現・調査できる発展案です。

## VOWN-01：modelごとのallocation領域

文書、simulation、履歴等のmodelにallocation領域を関連付ける。どのmodelが何を保持するかを値の周辺情報として持ちたい。

領域終了時に残るsnapshotを明示し、全体GCとは別に回収可能な部分を知る。外部arenaのphysical寿命とは区別する。

## VOWN-02：copy/clone/moveの費用表示

同じ代入や引数でも、共有、深いcopy、owner移動のどれかをeditorで確認する。大きい値をhelperへ渡したときの費用を理解したい。

利用者が操作を選べるAPIを揃える。見た目だけ短いcodeが、毎回全graphをcopyする結果にならないようにしたい。

## VOWN-03：disjoint borrowの証明

異なるfield、非重複slice、別subtreeを、それぞれmutableに借用する。重ならないことが分かる場合、同じ親ownerだからというだけで操作を分けずに済ませたい。

静的に証明する方法と、checked splitで証明値を得る方法を使い分ける。indexから作るviewにも同じ発想を使いたい。

## VOWN-04：最後の利用で終わるborrow

借用の最後の利用が分かれば、その後は同じscopeでもownerを更新する。libraryの便利関数を使うたび、借用用blockを増やさずに済ませたい。

checkpoint、capture、defer、asyncで残る利用は別に追う。単にtext上の最後の名前参照だけで寿命を決めない。

## VOWN-05：snapshot-awareなCell

共有interfaceから限定的な更新を行うCellを考える。可変stateの中身とversionを関連付け、古いsnapshotの値を変更しない形にしたい。

普通のmutable aliasと同じ意味にせず、更新の権限やeffectを表す。callbackやreactive modelの小さなstateへ使いたい。

## VOWN-06：version-awareなweak reference

targetを保持しない参照を利用者のcodeへ提供する。取得時に、対象の存否とどの版に属するかを確認したい。

過去snapshotがtargetを保持することと、現在ownerとして使えることは分ける。無効なheap位置の再利用で別objectを返さない。

## VOWN-07：ephemeron型のcache

keyが他のrootから到達可能な間だけ、付随するcache値を保持する。keyとvalueが互いに参照しても、cacheだけで寿命を延ばさない構造が欲しい。

memoization、metadata、editor viewへ使う。checkpointでkeyが残る場合の会計も説明できると便利。

## VOWN-08：回収を知らせる通知

objectを復活させない弱い通知で、cacheや観測用metadataを整理する。GCがいつ動いたかをapplicationの正しさへ無条件に使わない方式にしたい。

physical resourceのcleanupは明示寿命を優先する。通知は取得済みobjectへの任意操作と分け、記録や再現性の扱いも選びたい。

## VOWN-09：一度だけ呼べるclosure

捕捉したownerを消費するclosureを、at-most-onceの型として渡す。callbackを二回呼ぶhelperへ誤って渡さずに済ませたい。

戻り値やerrorへownerを移す経路を表す。消費するResult helperやTask起動の契約とも組み合わせたい。

## VOWN-10：小さなvalueの配置選択

小recordやtupleを、必要がなければ個別heap objectにしない。checkpointへ保持された場合は、意味を保つrepresentationへ変換したい。

valueの公開identityと物理addressを区別する。配置最適化を使っても、debug表示や所有権の意味は変えない。

## VOWN-11：再利用poolとsnapshotの共存

短命objectを再利用するpoolを、userが保持するsnapshotと区別して使う。旧版が参照するslotは再利用せず、新しいidentityを発行したい。

poolの保持容量とlive容量をprofileへ分ける。固定rootの長時間実行で、無制限にpoolが成長しない方式が欲しい。

## VOWN-12：領域間のpromotion

一時領域から通常model、通常modelからcheckpoint外stateへ、値を明示的に移す。copy、immutable共有、owner移動を選択したい。

borrowだけを長寿命領域へ逃がさない。部分graphを移す場合も、残る参照と共有pageを確認できる操作にしたい。
