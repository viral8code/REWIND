# 構想を選ぶ前に比べたい設計の軸

状態：未決。現行仕様の変更、機能の採用、版の到達条件を宣言する文書ではない。

自由構想の数を増やすだけでなく、複数案を組み合わせた時に何を選ぶ必要があるかを残す。以下は独立した追加機能として件数に数えない。各判断は、現行実装との照合と最小利用例の比較を行ってから具体化する。

## 1：実行を戻すか、dataを戻すか

同じ「履歴」でも、関数・task・program counterまで含むcheckpointと、選んだmodelだけのSnapshot<T>では用途が異なる。

| 比較する案 | 得たい体験 | 比べる点 |
|---|---|---|
| 実行checkpointを操作 | 同じ実行地点で状態を戻す、再開する | 生きたscope、call path、task、owner、code版 |
| data snapshotを操作 | GUI callbackが終了した後のUndo、複数model比較 | 復元するmodel、参照、invariant、schema |
| 両方を持つ明示catalog | debuggerとdata履歴の連動 | 異なる操作を同名のrestoreへ隠さない |

[trial editor](01-trial-editor.md)はdata snapshotの用途。[scheduler laboratory](06-scheduler-laboratory.md)は仮想実行のstateを扱う用途。終了したcallback内のcommitを、後続callbackのUndo用dataとして使えると仮定しない。

操作名より先に、何を保存し、どのownerで保持し、どのscopeで使えるかを例にする。

## 2：checkpoint外stateを誰が所有するか

試行回数、候補catalog、receiptは通常modelを戻しても残したい。一方、物理budget、永続履歴、実行の説明では要求が違う。

| 比較する案 | 長所 | 比べる点 |
|---|---|---|
| runtimeごとのhyper binding | どのbranchからも同じ集計を使う | 多taskの更新、scope終了、begin |
| modelに付くretained owner | 独立modelを閉じると関連stateも閉じる | runtime共通receiptやbudgetとの境界 |
| 明示store parameter | 関数の依存と更新がsignatureに現れる | 毎回渡す記述量、helperの共通化 |

hyperに保存した値が、戻されるheapの一時borrowを持てるかは寿命の設計が必要。immutable snapshotのownerとして保持する方式と、選択dataをcopyする方式を比べる。

process再起動後の保持には別のstoreが必要。[effect workbench](05-effect-workbench.md)のreceipt registryと、disk上のoperation記録を同じ機能と数えない。

## 3：beginとretained stateの関係

現在のbeginに対する期待は「実行直後のクリーンな状態」。hyper領域の候補には「通常のrevertでは戻さない」という期待がある。追加仕様ではこの組合せを明示する必要がある。

比較するのは、beginもhyperを残す方式、beginに限って一部を初期化する方式、通常revertとは別のreset操作を用意する方式。

どの方式でも、既に実行した外部作用のreceiptや一度使ったnonceを、初期化したcounterだけで未実行にしない。runtimeの完全な作り直しも物理世界の初期化ではない。

具体例では候補一覧、試行数、実行budget、receiptを別々に置き、begin後に何が残るべきかを確認する。ここでいずれかに決めたとはしない。

## 4：branchのstateは共有か独立か

modelのbranchは独立でよいが、探索budgetは共有したい。cacheは共有できる場合があり、試行のrandom streamは対応を保ちたい。

| state | 比較したいpolicy |
|---|---|
| 通常model | fork時のroot共有、更新時copy-on-write |
| 乱数state | 共通prefixからの分岐、安定keyによる独立stream |
| metrics | 全branch共通、branch別集計、sampling |
| cache | input/code依存付きの共有、local限定 |
| budget | 全探索の物理budget、branchごとの計算上限 |
| 候補結果 | ownerを持つsnapshot、軽量な要約のみ |

[simulation tree](07-simulation-tree.md)と[inference explorer](10-inference-explorer.md)で、この違いを見える形にする。

「hyperだから全部共有」だけで済ませず、各stateが結果へ影響するか、保持費用を誰が払うかも確認する。

## 5：snapshotから出した参照の意味

Snapshot<T>から得たviewには、旧版の値を読む意味と、現在modelを編集する意味を混ぜたくない。

比較するのは、借用scope内だけのread-only view、snapshot ownerを保持する長寿命view、現在Entityへresolveするversioned reference。

dataは同じでも、現在Entityが消えていることがある。内容が同じでも、別runtimeのownerであることがある。

[typed form](09-typed-form.md)ではfield pathの型とmodel版を別に確認する。[native capsule](08-native-computation-capsule.md)ではphysical borrowをsnapshotの通常参照と区別する。

## 6：履歴をいつ削除できるか

「過去をdropする」「cacheをevictする」「pageを回収する」は別の判断。

履歴labelの登録解除後も、snapshot ownerがあればdataは残る。cacheから計算結果を消しても、利用者のpinした結果は残る。共有pageは最後のownerがなくなってから回収できる。

比較するUIは、labelの数だけを表示するものと、root・共有page・独自page・再計算可能性を表示するもの。

historyを要約する操作は、利用者が失う粒度をpreviewしたい。memory上限に達した時、意図して保持したrootを暗黙に捨てる仕様にはしない。

## 7：model採用の単位

単一field、invariant unit、model全体、複数model群のどこまでを一度に採用するかを選ぶ必要がある。

小さい単位は競合を減らせる一方、連動する制約を壊しやすい。大きい単位は整合性を示しやすい一方、関係のない変更も競合しやすい。

[typed path](../31-bidirectional-data.md)のChangeSetとfocus、[reactive graph](../36-reactive-models.md)の同一版通知を合わせて比較する。

taskが複数modelへ更新案を出す場合、どのownerが採用を直列化するかも決める。外部DB transactionをこのVM内の単位へ黙って含めない。

## 8：physical operationの実行境界

modelのpublish、Intentの採用、ticketの発行、physical実行、receiptの記録は異なる段階として考える。

~~~text
model edit
    -> validated model version
    -> Intent / Prepared
    -> authorized execution ticket
    -> physical operation
    -> receipt / unknown outcome
    -> current modelへの反映案
~~~

比較するのは、明示execute API、専用effect sink、operation workflow等の表現。どの表現でも、純粋なrecomputeがそのまま再送になる構成を避ける。

[effect workbench](05-effect-workbench.md)と[DBの結果](../40-database-contracts.md)では、Unknownを確認する必要がある。exactly-onceの強さはbackendが提供する能力に依存し、VM側のticketだけで一般に保証できるものではない。

## 9：cleanupの時間軸

local dataのrollback、continuationのdiscard、taskの終了、physical ownerのclose、GCによるdata回収には異なる寿命がある。

比較するのは、scopeに結び付いたcleanup、明示close owner、task groupのdrain、GC通知による補助的整理。

一度だけ必要な外部closeを、checkpointへ戻るたびに繰り返すhookにはしない。physical ownerが終了した事実を古いmodelで覆い隠さない。

[native capsule](08-native-computation-capsule.md)と[effect handler](../34-effect-handlers.md)で、unwindとdiscardの具体例を比較する。

## 10：code版をどこまでdataへ持たせるか

旧dataを読むだけならschemaだけで足りる場合がある。旧計算を再現するにはcode、backend、input、arithmetic context、handler等も必要になる。

保存する版情報が多ければ説明は強くなるが、保持するcodeやartifactも増える。軽量なdata packと、再現用run bundleを別formatとして比較したい。

[live module evolution](12-live-module-evolution.md)では旧frame、callback、opaque型のownerを確認する。[domain workspace](11-domain-language-workspace.md)では生成器版とorigin graphも対象になる。

「公開signatureが同じ」「data schemaが同じ」「同じ結果を再計算できる」を同じ互換性として扱わない。

## 11：決定性と測定を分ける

同じ入力で同じmodel結果を得る契約と、同じ仕事量で同じ物理時間になる期待は違う。

選択するmodeには、bit単位の再現、数値許容誤差内の再現、recordされた入力順の再現等がある。時計・乱数・task順・device reductionのどれを固定したかを明示したい。

metricsの時刻やmemory測定はphysicalな観測として残せる一方、それを計算の分岐へ使えば結果への依存になる。

[device計算](../44-device-computation.md)と[数値contract](../32-numeric-contracts.md)で、速度と保証の違いを比較する。

## 12：便利な既定と見える費用

syntaxを短くすると、copy、snapshot保持、dispatch、再計算、transfer等を見落としやすい。すべてを毎回sourceへ書くと利用負担が増える。

比較するのは、明示APIだけの設計、通常の既定にeditor annotationを付ける設計、型・effect・planを必要時に展開する設計。

簡単なcodeは簡単に書けて、必要な時には実際の費用と操作境界を確認できる体験を目指す。既定のannotation生成が、高いobserver overheadを常に追加しないようにしたい。

[Diagnostic](../37-diagnostic-values.md)と[backend閲覧](../28-runtime-backends.md)から、利用者向け説明と低level説明をつなぐ。

## 比較用の小さいscenario

後で採用案を絞る時は、以下を短い例にして比較する。新しい受入条件をここで追加したという意味ではない。

1. 画面callbackが終わった後にUndoする。戻すdata、残すreceipt、表示する入力bufferを説明できるか。
2. 失敗する候補を100回試す。通常modelは戻り、budgetと失敗要約は残り、不要なrootは解放できるか。
3. background計算中に入力を変更する。旧結果を識別でき、採用・破棄・再計算のどれを選ぶかが読めるか。
4. DB commit要求後に接続が切れる。Unknownを照合し、modelのrevertをDB rollbackと誤解しないか。
5. native/GPU計算中にcancelする。buffer owner、完了確認、回収までの寿命を説明できるか。
6. 旧schema/codeのsnapshotを開く。閲覧、変換、実行再現、現在modelへの採用の違いを説明できるか。
7. beginへ戻る。通常値、候補履歴、budget、nonce、receiptの扱いが選んだ仕様で一貫するか。
8. 多数のcheckpointを保持する。意図したhistory保持と、一時値やcacheの解放漏れを区別できるか。

回答がまだ決まらない場合は選択肢のまま残す。自由構想を早く実装するために、曖昧な意味を確定済みとして書かない。
