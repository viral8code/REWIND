# DBの読取り版・query・更新の契約

既存DB接続の上に、queryの型、transactionの境界、観測dataの再利用をどう表すかを考える。DB transactionとVM checkpointを同じ操作として扱わない前提を維持する。

## VDB-01：queryとparameterの型

結果row型に加え、parameterの型とNULL可否をQuery<P, R>へ持たせる。名前・順序の違いをbind時に確認する。

実行前に検査できるschema情報と、接続先で初めて分かる情報を分ける。SQL文字列の埋込みはparameter bindingとは別操作にする。

## VDB-02：読取り版のidentity

DB snapshot、transaction、LSN等、backendが提供する読取り基準をReadEpochとして返す。

同じReadEpochの複数queryがどの整合性を持つかを表示する。backendが提供しない一貫性を、VMのsnapshot型だけで保証しない。

## VDB-03：row decodeの失敗位置

column名、rowの識別情報、期待型、実際のDB型をdecode errorへ持たせる。値を表示するかはredaction policyで選ぶ。

一行失敗で止める、失敗行を別streamへ出す、部分結果を返す等のpolicyを明示する。失敗を黙ってNULLへ変えない。

## VDB-04：row bufferのlease

DB driverの再利用bufferを短期viewとして借り、必要な値だけownerへcopyする。大きなblobを毎行無条件にcopyしないAPIを考える。

cursorが進むと無効になるborrowを型で追う。checkpointへ残したいrowは明示的にcaptureし、physical cursorを保存しない。

## VDB-05：paginationの継続条件

keyset cursorへsort key、tie-breaker、filter、schema版を付ける。別queryへ同じcursorを渡す誤りを減らす。

更新されるtable上での重複・欠落の条件も示す。backendが固定読取り版を提供する場合と、最新dataを読み続ける場合を区別する。

## VDB-06：statement planの寿命

prepared statementが接続、schema、parameter型へ依存することをownerで表す。切断後やschema変更後の再prepareをlibraryへ集約したい。

plan cacheの利益と保持資源を見えるようにする。VMのrevertで古いphysical statementを再び有効にしない。

## VDB-07：transactionを返す結果

Committed、RolledBack、Unknownを、backendの観測可能な状態に応じて返す。commit要求後の通信断を通常の失敗へ潰さない。

transactionで確定したDB状態と、modelへの反映を別段階にする。VM側が後で戻ってもDBのcommitを消したことにはしない。

## VDB-08：retry可能な更新program

serialization failure等に対するretryを、再実行可能なDB処理だけのcontractへ限定する。

network送信、nonce発行、外部通知等を同じretry blockへ無条件に入れない。attempt回数・budget・前回の結果はcheckpoint外の実行記録へ持つ。

## VDB-09：queryのfixtureと記録

query、parameter、読取り基準、結果schemaを記録し、純粋modelの試験で使う。DBを再接続せず、同じ観測入力を供給したい。

fixtureの対象範囲外なら明示的に失敗するmodeを用意する。実DBへのfallbackで再現testが知らないうちに外へ接続しない。

## VDB-10：schema差分からmigration案

schema間の差をtyped planにし、data変換・index・制約・停止条件をpreviewする。renameとdrop/createを利用者が区別できる。

migrationの逆案が作れる場合と、dataを失って逆に戻せない場合を明示する。model上の試行と実DBでの実行結果は分けて記録する。

## VDB-11：query planの費用観測

DB内のscan、転送量、decode、VM側集計を分けて測る。queryを短く書けたことと、効率よく実行されたことを区別したい。

実DBのplanとlibraryの推定planを同じ精度の情報として扱わない。driverの能力に応じたexplainを返す。

## VDB-12：backendへ押し込む変換

純粋なfilter、projection、集計をqueryとしてbackendへ送るか、VM streamで評価するかをplanで選ぶ。

NULL、collation、decimal、時刻等の意味の差を検査する。意味が一致しない場合はVMで実行し、転送費用との比較を利用者が選べるようにしたい。
