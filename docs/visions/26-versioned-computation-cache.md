# 版を跨ぐ計算cacheと再計算

reactive modelや永続subtreeのcacheより踏み込んで、どの計算を何に基づいて再利用できるかを利用者が扱う自由構想です。

## VCACHE-01：読んだ値を記録するpure call

結果だけでなく、入力・捕捉値・参照model versionを記録する。関数の再実行が必要かどうかを、依存情報から判断したい。

読取りがhiddenな外部観測ならpure扱いにしない。利用者が依存関係をqueryできるcall receiptを考える。

## VCACHE-02：keyの意味を選ぶcache

object identity、content、特定field等をkey policyとして指定する。同じ形でも意味の違う値を混同せず、逆に同じcontentの別objectは共有したい。

keyの作成費用と比較費用を含めて測る。巨大graphを毎回hashするcacheを、無条件に高速化とはしない。

## VCACHE-03：multi-version memo

過去版と現在版の計算結果を同じcacheへ持つ。branchを行き来したときに、前に得た結果へ戻れるようにしたい。

どの版が何を保持するかをprofileで見る。user snapshotと、失効してよい計算cacheは別のroot policyを使う。

## VCACHE-04：回復可能な失敗のcache

同じ不正入力に対するvalidation等のErrも再利用する。成功値だけをcacheし、何度も同じ失敗計算をする負担を減らしたい。

一時的なnetwork failureや消費予算超過を同じ扱いにしない。失敗がどの前提に依存するかを結果へ添える。

## VCACHE-05：差分から結果差分を作る

入力のChangeSetを受け、全計算ではなく結果の変更を返すoperationを定義する。利用者がincremental版と参照版を同じinterfaceで使いたい。

適用できない差分なら全再計算へ戻る。結果のold/new versionと、利用した変更の範囲を説明する。

## VCACHE-06：cached resultの所有権

cached値をimmutable共有するか、新しいownerとして渡すかをAPIへ表す。cache hitで得た可変値を変更して他callerの結果を壊さないようにしたい。

freeze、copy、thawの費用も見たい。元関数が毎回別identityを生成する意味を、cacheで暗黙に変更しない。

## VCACHE-07：明示的な遅延値

まだ計算していない値を、入力と計算の定義を持つLazy<T>として扱う。必要なときにforceし、その仕事量と失敗を通常の結果で受けたい。

debug表示や任意getterが勝手にforceしない。snapshotへ保持できる捕捉と、保持できないresourceを区別する。

## VCACHE-08：pipelineの中間結果共有

同じ前処理を使う複数の出力へ、中間値を共有する。どこまでmaterializeするか、何を再計算するかを利用者が選びたい。

全入力のコピーと、必要なchunkだけの共有を分ける。記録・profile上も各stageの費用を示す。

## VCACHE-09：費用に基づくcache policy

key作成、lookup、copy、再計算を比較し、cacheする価値のある区間を選ぶ。policyの選択理由を表示し、固定policyも使いたい。

cache hitでも実際のlookupやcopy費用は会計する。存在しない再計算をしたことにする数字や、費用を無視する高速化にはしない。

## VCACHE-10：再計算理由の説明

「このfieldが変わった」「codeが更新された」「容量でevictされた」等を表示する。期待よりcacheが効かない原因をsourceとdataへ辿りたい。

巨大な依存graphはsummaryから調べる。計測しないときの追加費用を小さくしたい。

## VCACHE-11：検証可能なoffline cache

pureな計算結果を、input hash、code/library version、formatと一緒に保存する。別実行で再利用する場合も、必要条件をloaderが照合したい。

保存済み結果にnative handleを混ぜない。署名と内容の検証、schema移行、欠損時の再計算を選ぶ。

## VCACHE-12：cacheだけを選択的に失効させる

modelを変更せず、依存条件や容量によってcache entryだけを無効化する。過去snapshotが保持する値は残したまま、調査用の再計算を選びたい。

entryの登録解除と、shared payloadの実解放を区別する。hyperな領域へcacheを置く場合も、versionの前提を守る。
