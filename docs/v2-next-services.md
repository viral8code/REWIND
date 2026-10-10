# 次版構想：通信・サービス・外部資源

状態：採用候補、未実装。版番号は未割当です。[全体計画](ROADMAP_v2-next.md)と[v2.2 DB計画](REWIND_v2.2-plan.md)を土台にします。

HTTP/HTTPS client、TCP、TLS付きHTTP server、routing、Secret credential、SQLite/PostgreSQL、非同期処理、取消、cleanupは既存です。ここでは、それらを組み合わせるときの不足を調べます。接続や送信済み内容をVM revertで復元しません。

## NET-01：URL・query・pathの型付き操作

まず現行HTTP URL解釈と`std.path`を調べます。filesystem pathとURI pathを混同せず、URL解析、percent encoding、queryの複数値、相対URLの解決を一貫したAPIで扱う案です。

encoding済み/未encodingを区別し、二重decodeを避けます。queryには同名parameterと空値を認める場合の順序を規定します。Unicode、IPv6、port、fragment、userinfo、不正escapeを検証します。URL中のcredentialは既定診断へ出しません。

**受入：** parse/formatとresolveの標準例・境界例。route用pathとfilesystem用pathを混用しない。URL builderがnetwork送信を起こさない。

## NET-02：header・cookie・bodyのprotocol helper

header名の大小文字、複数値、禁止改行、Content-Type、Cookie/Set-Cookieを明示したpure helperを検討します。まず既存native HTTPの処理と制約を読み、二重のprotocol実装にしません。

body候補はform-urlencoded、multipartのbounded encoder/decoderです。boundary分割、stream途中のdelimiter、総容量/field数/各field容量を扱います。upload filenameをそのままFile pathに使いません。圧縮は展開量上限とnative work会計を設計できる場合だけ別単位で追加します。

**受入：** 小chunkでも境界を正しく処理し、巨大fieldを拒否する。parser失敗で接続資源を漏らさない。認証header/cookieをtraceやerrorへ平文で残さない。

## NET-03：明示retry policy

現在のclientに自動retryを追加する仕様ではなく、利用者が選ぶpolicy/helperを候補にします。対象error、最大回数、全体deadline、backoff、jitter、再送可能なbody、idempotency keyを明示します。

送信前失敗、応答受信済み、送信後の結果不明を区別します。GETも副作用がないとは無条件に判断しません。POST/DB書込みのunknownは利用者の照合なしに再送しません。delay/jitterは既存time/randomと記録契約を確認して実装します。

**受入：** timeout/cancelで待機を終了し、retry stormを起こさない。replayは記録を読むだけで送信しない。fresh再試行だけが新しいphysical操作になる。

## SVC-01：routerの構成と入力schema

`std.httpRouter.find`はexact path/methodの線形探索です。この実装を残し、parameter付きroute、method判定、typed path/query/body変換を別のcompiled router候補として設計します。

route登録時に重複・曖昧な優先順位を検出します。pathのdecode段階、slash、trailing slash、404/405、HEAD/OPTIONSの扱いを固定します。handlerをString名で識別する現行方式と、effect付きcallbackを持つ方式の成立性を確認します。認証やDB transactionをrouterが自動実行しません。

**受入：** 明示した優先順位で解決する。要求の数・長さ・深さに上限がある。LANG-06の検証結果から、機密値なしの入力errorを返せる。

## SVC-02：request scope・取消・graceful shutdown

requestごとの子task、deadline、終了時のcleanupをまとめる案です。既存Task/select/yield/cancelの意味を確認し、親終了で子を取消するscopeの契約を決めます。戻り値にnative ownerを含むときの回収も設計します。

shutdownは新規受付停止、処理中要求の猶予、取消、DB/socket cleanup、終了という段階を持ちます。VM logical timeoutとphysical接続deadlineを一つに見せません。内部timer機構やsignal入力が足りなければ、その観測境界を独立した設計へ分けます。

**受入：** slow client、応答途中、子task失敗、期限到達、二重shutdownを確認する。未応答要求を成功200と扱わず、残るresourceとunknown書込みを説明する。

## SVC-03：structured logと運転中の統計

診断・profile・recordは既存ですが、アプリケーションが任意の分類で残すlogとは役割を分けます。level、code、request/task ID、source、causeを持つbounded eventを候補にします。

logをVM内bufferへ追加するのか、external領域で運転logを出すのかをAPI名とeffectで分けます。revertで取消可能なlogと公開済みlogを混ぜません。既定では値を載せず、Secret/redactionを共通化します。queue容量、overflow時の省略件数、flush失敗を定義します。

統計はcounter/gauge/histogramを少数のlabelで扱う案です。任意入力をlabelにして無制限にseriesを作りません。profile counterと業務統計を二重課金・混同しない設計にします。

**受入：** 高頻度eventでも容量制限を守る。replay中に運転logを再送しない。機密値と高cardinalityの入力を既定で保持しない。

## SVC-04：credential更新と接続再利用

現行immutable credential aliasを変更可能な共有値へ置き換えません。新credentialの登録、新規接続への切替、旧接続の終了を明示する案です。expiryと登録先の寿命を調べます。

connection poolはDB計画で保留中です。追加する場合は最大接続、待機者数、貸出期限、取消、壊れた接続の廃棄、credential世代、reset検証を先に決めます。VM revertでphysicalな貸出履歴が戻るhandleを作りません。

**受入：** 旧credentialの接続が意図せず再利用されない。待機取消とshutdownで接続が戻る。poolのmemory/handle会計をruntime上限へ含める。

## NET-04：長い双方向通信

WebSocket等は候補です。既存TCPでapplication protocolを組めることと、標準protocol libraryを提供することは分けて判断します。

最小範囲はhandshake、frame、fragmentation、ping/pong、closeです。message容量、frame容量、送受信queue、idle deadlineを規定します。圧縮・自動再接続・多数の拡張は後続候補です。received messageの観測と、sent frameの不可逆作用を区別します。

**受入：** 分割frame、不正mask/length、途中close、cancel、slow peerを扱える。replayで接続を再開しない。実用例がなければprotocol追加を保留する。

## 優先順位と依存

NET-01 → NET-02 / SVC-01 → SVC-02 → NET-03 / SVC-03 → SVC-04 / NET-04。

SVC-01は言語構想LANG-06の検証方式と整合させます。SVC-02はv2.1のtask/cleanup診断を利用します。SVC-04はDB-02の終了状態を利用します。後半のpool/双方向通信は必須公開条件にしません。

各採用候補はpure unit、record/replay、使い捨てlocal serverによるphysical確認を分けます。外部送信を繰り返す負荷試験はdefaultで起動しません。公開CI/CD停止中はローカル検証の範囲を守ります。
