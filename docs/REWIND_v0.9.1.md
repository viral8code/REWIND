# REWIND v0.9.1 草案 — アプリケーション開発に向けた不足

作成日: 2026-10-01。状態: **P0・初回完了条件を実装、P1/P2 は後続候補**。基準は [v0.9 実装状況](v0.9-status.md)。本書は「アプリを作る際に何が必要か」を整理したもので、候補全部を次の一回で実装する約束ではない。

## 1. 現在作れるものと境界

REWIND の中心は、計算・所有権・論理 task・仮想 I/O を Checkpoint で戻し、記録を決定的に再生することにある。v0.9 では型・効果・署名済み依存・source-free artifact・test・文書・編集支援まで揃えた。アプリの通信・UI・業務データ基盤はまだ揃っていない。

| 作りたいもの | 現在使えるもの | 実用化を妨げる主な不足 |
|---|---|---|
| 小さなオフライン CLI / バッチ | Args/Env/In、文字列・数値、File/Directory、Result、署名付き artifact | JSON/CSV、引数 schema、設定検証、exit code、asset 配布、業務エラー表示 |
| ローカルのメモ・家計簿・管理ツール | collection/record、ファイル仮想更新、Checkpoint | 安定した保存 schema、migration、ロック、複数ファイルの整合性、検索・DB |
| Web API / Web アプリ | 論理 task・Channel、純粋な domain 処理 | HTTP/TLS、socket、Host event loop、認証、DB、deploy、実時間 deadline |
| デスクトップ / mobile GUI | 型付き state と変更を戻せる domain 処理 | GUI toolkit、入力イベント、描画、platform bridge、配布・署名 |
| 外部 API と連携する自動化 | virtual I/O と観測の replay、原因木 | HTTP client、rate limit、retry/idempotency、credential 管理、外部副作用の境界 |
| ライブラリを組み込むアプリ | Rust Runtime library、public API snapshot、verified artifact | 安定した compiler/embed API、Host adapter、ABI/FFI/Wasm interface、互換期間 |

オフライン CLI を最初の目標にする。Web/GUI の土台まで同時に追加すると、外部の副作用と rewind の契約が曖昧になるため、Host 連携は別の設計段階で定義する。

## 2. 最初に採用したい範囲（P0）

### 型付きデータと設定

`Json` の AST、parse/stringify、UTF-8、Int/Float、null、object key の順序・重複・数値 overflow・非有限値を定義する。初版は明示変換 API を使い、任意の型の自動 reflection は要求しない。parse の byte/depth/item 予算と位置付き JsonError を持つ。record/enum の codec は schema を明示し、秘密値の通常 stringify は拒否または redacted とする。

設定は JSON と Env/Args の優先順位を固定し、必要項目・型・未知の項目・default を検査する。エラーは Result と Diagnostic に位置・項目名を残す。設定の読み直しは外部観測として記録し、replay では Host から取り直さない。

完了条件: config JSON を読み、record の domain state へ変換し、更新を保存するオフライン CLI の例が実行・記録・replay できる。不正 UTF-8・数値・深さ・未知項目を別々に診断する。

### アプリとして終了し、失敗を伝える

`main` の入口・戻り値、検査エラー/業務エラー/panic/予算超過/取消しを区別する exit status を定義する。既存スクリプト入口は維持する。ユーザー向けの短い表示と開発者向けの typed cause/JSON を選べるようにする。機密値・設定の秘密項目を原因木に出さない。

Host への publish 前に失敗したケースと、PublishPartiallyApplied のケースを区別し、自動 retry の可否を判断できるようにする。

### asset を含む再現可能な配布

production install に明示した asset の allowlist と digest を追加する。source 以外の画像・template・schema・設定 sample を含められるようにする。秘密鍵・credential・cache・record は既定で配布しない。manifest の release entry、compiler/target、resource budget、必要 effect を成果物に記載する。

完了条件: 新しい directory に source/package/asset を配置し、開発用 package なしで実行できる。asset の欠落・改変・path traversal・symlink を診断する。署名と digest を保った build が再現できる。

## 3. 保存アプリで先に必要になる範囲（P1）

### 永続化と復旧

record/enum を保存する際の schema version、未知フィールド、enum variant の増減、migration を定義する。Checkpoint は VM の履歴であり、永続 DB の transaction と同義ではない。複数 process の書込み競合、file lock、atomic replace、crash recovery と backup を別の契約にする。

現状の publish は複数ファイル/stream に対して globally atomic ではない。初版の保存形式は一つの atomic replacement にまとめるか、回復可能な journal を持つ。DB adapter は transaction commit の不可逆性と idempotency を明示し、revert が外部 commit を取り消すと見せない。

### 日時・業務データ・標準ライブラリ

日時/期間/timezone/locale、money の decimal、文字列正規化・比較、path の検証、collection の検索・sort・iteration の API を整理する。Float を金額へ無条件に使わない。精度・overflow・並び順・入力上限をテストする。現在の Time 観測と scheduler の論理 step timeout を、実時間の時計・deadline と区別する。

### 開発体験と長い session

新しい未検査 declaration の provisional header 解析、workspace/test mode、import alias 編集、project scaffolding、CLI help と editor integration を整える。長い REPL は prefix 再実行のコストが増えるため、計算/型検査/cache の profile と session 単位の上限を示す。

transcript の source/通常観測は機密データを含み得る。Secret の別供給・拒否条件・signature・保存 policy を定義する。暗号化を導入する場合は認証付き暗号と鍵管理の仕様を先に作る。source_signature_verified という説明フラグを authenticity の証明と混同しない。

## 4. Web/GUI/外部連携に必要な設計（P2）

### Host adapter の境界

HTTP・DB・GUI を VM 内から無制限に呼べる API にしない。Host が提供する adapter の request/response schema、effect/capability、許可対象、上限、秘密項目を宣言する。adapter による観測は journal に記録し、replay は adapter を呼ばない。外部への書込み・送信・DB commit は明示した publish 境界で扱う。

request ID、idempotency key、retry、部分適用、切断・timeout・取消し、応答順を定義する。外部サービスの transaction を VM の Checkpoint と同時に巻き戻せると仮定しない。HTTP の GET であっても外部副作用の有無は adapter 契約に従う。

### イベント処理と UI

Host event loop と scheduler の接続、入力イベントの順序、backpressure、実時間 timer、取消し後の cleanup、session の終了を定義する。描画は domain state の projection とし、復元後に再描画できるようにする。Host の file dialog、clipboard、通知、window lifecycle は観測/外部操作として区別する。

初めは Rust Host に REWIND の domain 処理を組み込む例を検討する。安定した compiler/runtime 埋め込み API を公開し、GUI toolkit や HTTP server の実装は Host 側の既存ライブラリを使う。FFI・OS thread・Wasm はこの契約を検証してから採否を決める。

### 運用・配布

構造化 log、metrics、trace redaction/retention、診断の correlation ID、設定・credential の更新、health、graceful shutdown を設計する。signature の trust/key rotation と compiler 更新時の artifact/trace 再生成手順を用意する。最低限の CI、cross-platform package、install/uninstall、更新/rollback の例を作る。

## 5. v0.9.1 の初回完了条件

最初の実装単位を「JSON 設定を読み、型付き state を変更し、単一ファイルへ保存するオフライン CLI」とする。

1. JSON の parse/stringify と予算・位置付き失敗を実装する。
2. 設定 schema・引数・業務エラー・exit status を提供する。
3. 明示した asset を production 配布し、署名・digest を検証する。
4. 正常系、不正入力、保存競合、publish 失敗、record/replay、秘密表示を例と回帰で検証する。

HTTP/DB/GUI、borrowed return、動的 trait object、暗号化 trace はこの初回の完了条件には含めない。対応する P1/P2 の仕様と実例を作り、次の変更単位を小さく決める。

## 実装への対応

2026-10-01: P0 と第5節の初回完了条件を採用した。具体的な API・上限・互換性・保存/配布の制限は [v0.9.1 実装状況](v0.9.1-status.md) を参照。標準ライブラリを整備し、SDK 配布と次の library 拡張を [v0.9.2 草案](REWIND_v0.9.2.md) に分けた。process 間 lock・migration・DB/HTTP/GUI はこの版の採用範囲外。
