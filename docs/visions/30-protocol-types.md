# protocol・資源の状態を表す型

session typeや所有権の構想を、利用者が組み立てるAPIへ広げる。以下は未実装の発展案で、現行libraryの有無を判定した一覧ではない。

## VPROTO-01：状態ごとに操作が変わる資源

Connection<Disconnected>、Connection<Authenticated>等の状態を型引数にし、認証前の送信や終了後の読込みを呼出し時点で避けたい。

状態遷移は元のownerを消費し、新しい状態のownerを返す。外部接続が実際に終了した後に、VMのrevertで有効な接続へ戻せることは意味しない。

## VPROTO-02：初期化順を検査するbuilder

必要なfieldを埋めたかを型で記録するbuilder。URLと認証方式が揃った場合だけbuildが現れ、任意fieldには既定値を置ける。

必須fieldの順序に不要な制限を付けず、部分的な設定を別関数へ渡しても、残りの義務をsignatureから読めると便利。

## VPROTO-03：分岐するprotocolの返値

応答が成功・再認証・redirect等へ分かれる時、次に許される操作も各variantへ持たせる。成功値だけでなく継続用ownerが返る。

すべての分岐に対する処理をmatchで確認する。共通の後処理へ状態の違いを隠すなら、その時点で失われる操作能力を明示する。

## VPROTO-04：役割を反転するprotocol型

client/serverの送受信順を一つのprotocol記述から得たい。送信と受信、選択と分岐を反転して、両端の契約が対応しているか調べる。

wire versionやoptional extensionの違いも表示する。型が対応していることと、実装や配送が正常であることは別の確認事項として扱う。

## VPROTO-05：交渉後の能力を持つsession

接続時に交渉した圧縮・サイズ上限・stream数をSessionCapabilitiesとして返し、それを要求する操作へ渡す。

利用者は実際に得た能力に応じてalgorithmや転送方法を選べる。compile時の対応能力と、今回の接続で有効な能力を区別する。

## VPROTO-06：payloadの所有権移送

messageへbufferを移す、読取り専用で共有する、copyする、の契約を送信APIで表したい。送信完了まではbufferの寿命を守る。

完了が所有権返却なのか、配送確認なのか、相手側処理完了なのかも分ける。単一のdoneで複数段階を混ぜないlibrary設計を考える。

## VPROTO-07：取消しにも状態遷移を持たせる

requestの取消し後をCancelled、Completed、Unknown等の型付き結果として返したい。cancelを呼んだ事実だけで未実行と判定しない。

再利用できるchannel、閉じる必要があるchannel、照合待ちのoperationを返値から扱えるようにする。

## VPROTO-08：callback登録をleaseにする

callbackの登録期間をownerで表し、解除済みの登録を使えないようにしたい。callbackがcaptureした値の寿命もleaseに結び付ける。

既に実行中のcallbackを解除が待つか、完了を別途待つか、再入を許すかを契約として読みたい。

## VPROTO-09：group単位の終了手順

複数streamやchild sessionを持つ資源に、受付停止、drain、closeの段階を用意する。終了前に必要な義務を型または検査可能な一覧へ残す。

drainの期限に到達した場合は、未完了の対象と観測結果を返す。終了手順をrevertして外部送信を再開したことにはしない。

## VPROTO-10：純粋なprotocol modelとの対応

物理sessionとは別に、同じ状態遷移を使う純粋modelを生成したい。記録済みeventや故障注入で遷移を調べられる。

modelのbranch探索結果を実sessionへ移すのではなく、実sessionの観測を照合する判定器やtest fixtureとして使う。

## VPROTO-11：protocolの部分合成

認証・転送・heartbeat等のprotocol fragmentを合成し、状態名の衝突や互いの待機条件を確認したい。

fragmentごとのlocalな正しさだけで全体のdeadlock不在を宣言せず、合成後の待機graphも見えるようにする。

## VPROTO-12：義務が残る値の診断

close、ack、confirm、rollback等を要求するownerがscopeを出る際、その義務と生成場所を説明する診断を考える。

自動cleanupが使える操作と、結果を利用者が判断する操作を分ける。GCの到達不能判定だけで業務上の確認が済むと解釈しない。
