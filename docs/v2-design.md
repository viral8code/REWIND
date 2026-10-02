# v1.5〜v2.0 詳細設計案

これは v1.4.0 を基準にした設計案。v1.5 の基盤は実装済みで、各版の個別文書に実装状況を記す。[ロードマップ](ROADMAP_v2.md) の到達条件を実装と検証へ分解する。以下の新しい構文・API 名は提案であり、現在の処理系で利用できるとは限らない。

## 1. 変更対象と実装順

| 変更 | 主な対象 |
| --- | --- |
| 構文・型・effect | `src/v2.rs`、`src/v2/effects.rs`、`src/v2/v06.rs`、`src/v2/v06/effects.rs`、`src/v2/project.rs` |
| VM とタスク | `src/v2/vm.rs`、`src/v2/vm/scheduler.rs`、ownership / captures の各 pass |
| 状態・予算・GC | `src/lib.rs`、`src/storage.rs`、`src/map_storage.rs` |
| 観測と replay | `src/replay.rs`、artifact / cache / trace の各処理 |
| native adapter | 新規 external / network / db / numeric module。host 資源を Runtime に集約 |
| 標準 API・SDK | `libraries/std/*.rw`、`src/v2/v092.rs`、`src/v2/v092/sdk.rs`、`libraries/api` |
| GUI・tooling | `src/gui`、LSP / formatter / API tooling、言語リファレンス |
| 検証・配布 | `tests`、`examples`、benchmark 資材、`scripts/package-sdk*`、CI / Release workflow |

新しい statement を追加する場合は parser / checker / compiler だけで終わらせない。rename、import resolution、効果推論、所有権、closure capture、cleanup、LSP、artifact、cache、REPL の AST walker をすべて点検する。新しい native 値も GC root、serde、型判定、等価性、Send / Share、予算に反映する。

変更は以下の順に組む。

1. 外部境界と操作記録を、単純な時刻観測・テスト用 adapter で確立する。
2. typed result と資源 registry の共通層を作り、HTTP を縦に実装する。
3. host I/O の待機・キャンセルを task と統合してから接続型 DB を加える。
4. 数値 storage と kernel を入れ、GC と COW の検証を同時に追加する。
5. 各段階で UI 応答・memory を測り、1.9 で全体監査する。性能検証を 1.9 まで先送りしない。
6. 2.0 の統合ケースから逆算して残る言語・ライブラリの不足を修正する。

## 2. 巻き戻しと外部作用

### 2.1 状態の区分

| 状態 | checkpoint | revert / begin 後 |
| --- | --- | --- |
| VM 変数、heap、task の計算状態、数値 buffer の root | 対象 | 保存した root に戻す |
| 未公開 file / output / GUI scene | 対象 | 未確定差分を戻す |
| 観測の読取り位置、外部操作の論理位置 | 対象 | 保存した位置に戻す |
| 観測済み結果、外部操作の成否、送信結果不明の記録 | 対象外 | 保持し、同一操作を再送しない |
| publish 済み output / file / GUI | 対象外 | 消去・再送しない |
| socket、DB connection、OS window、IME composition | 対象外 | 同じ生存中資源を参照するか、失効エラーにする |
| 消費済み work、実時間 timeout、接続・操作の物理 ID | 対象外 | 巻き戻して増やしたり再利用したりしない |

VM の rollback と外部 transaction は独立。`revert begin` も公開済み事実を消さず、閉じた connection を再作成しない。ユーザーコードを先頭からやり直す resume でもこの区分は同じ。

### 2.2 明示領域

提案は `external { ... }` と `external fresh { ... }`。必要な `external` と `network` / `db` 等の effect を manifest / CLI でも許可する。region は許可を自動付与しない。

- 通常の external は、同じ論理位置・同じ要求なら記録済み結果を返す。新しい位置なら外部操作を実行する。
- fresh は「ここから先は新しい操作」と明示する。配信済みの操作位置へ進める。replay 用に事前読込みした全記録の末尾へ進めてはいけない。
- 同じ位置で要求が変われば `ExternalRequestMismatch`。自動的に別操作として送らない。
- region 内でも通常の変数操作は VM 状態として扱う。region の終了を publish と解釈しない。
- checkpoint / revert / resume / branch / publish は region 内で拒否する。関数・closure 経由にも runtime guard を置く。
- region をまたぐ return / `?` / break / continue は exit を確実に通す設計が必要。初版で未対応なら静的に拒否し、helper 内の通常の return と区別する。1.9 までに必要な unwinding を完成させる。
- await による suspension は、task ごとの region context と操作 state machine が揃うまでは拒否する。Runtime 全体の単一 bool を複数 task で共有しない。

読み書きの結果は Result と通常の不変データへ変換する。API の effect と「external 呼出し位置が必要か」は別に推論する。自分の内部で region を完結させる関数に、呼出し側の入れ子 region を強制しない。

### 2.3 操作の識別と順序

操作記録は最低限、論理操作 ID、adapter / schema 版、要求 fingerprint、資源の論理 ID、結果 / 失敗区分、完了状態を持つ。

- 初版を main task の逐次操作に限定して安定させてもよい。task 対応時は task / branch の論理 ID と操作 sequence を使い、別経路の同じ sequence が誤って衝突しないようにする。
- checkpoint は sequence を保持する。外部の物理 ID・操作の完了記録は巻き戻さない。task を再生成した場合の論理 ID と、host worker の新しい物理 ID は区別する。
- 要求には method / URL / SQL / parameter / body・選択した制限など、結果に影響する公開入力を含める。組立て順による誤判定を避けるため canonical な表現を定義する。
- credential の平文、認証 header、秘密 parameter は記録・診断へ出さない。credential は明示した alias で参照し、秘密部分の照合が必要なら鍵付き digest と鍵の扱いを設計する。単純な平文 hash を秘密保護と呼ばない。
- 外部操作の失敗は「送信前に未適用」「完了が確認された」「部分適用」「適用結果不明」を分ける。timeout / cancel は未適用の証明ではない。
- in-flight な操作へ戻った場合も worker を再発行しない。既存操作の待機、キャンセル要求、結果の配信を操作 state machine で処理する。

自動再送は既定で禁止する。HTTP client / DB driver 自体の hidden retry も点検する。意図的な再試行は fresh と adapter の retry API による新規操作であり、外部で二重適用されないことまで保証しない。

### 2.4 記録の費用と replay

外部システム全体の snapshot を取らない。操作単位の必要な結果だけを記録する。

- 送信前に記録枠・最大結果容量・work を予約する。容量不足なら host を呼ばない。
- 成功後に記録できなかった操作は `OutcomeUnknown` として保持し、revert 後にも自動再実行を拒否する。永続的な crash recovery を保証するものではない。
- body / row batch は参照共有した immutable block で保持し、revert や値 clone で深くコピーしない。大きい記録は既存の spill 方針と整合させる。
- journal / operation metadata にも memory / storage 上限を適用する。各 API の byte / row / deadline 上限と、Runtime 全体の累積予算を分ける。
- 初版は明示した上限を超えると止める。1.9 までに、どの checkpoint / branch / task / 保存 trace からも再参照されない prefix を安全に解放する設計を加える。削除された ID を別操作に再利用しない。
- 秘密を含む結果は既定の trace から除外する。除外された値が後続計算に必要なら、その trace の完全 replay は拒否するか、別の秘密入力を要求する。値を欠落させたまま再現を保証しない。
- replay は接続・送信・書込み・server listen を実行しない。入力と操作の照合だけで同じ結果を返す。記録が尽きた際に live 通信へ fallback しない。
- in-process の二重実行防止と、プロセス再起動後の exactly-once は別。後者を v2.0 の既定保証にしない。

大容量・長時間処理用に、結果を再利用しない `external live` 相当の選択肢を評価する。追加する場合は明示的 opt-in、巻き戻しによる再実行、完全 replay 非対応を型 / 診断 / API に表し、既定の recorded 操作の契約を弱めない。性能測定で必要性がなければ構文を増やさない。

## 3. 外部資源と非同期実行

### 3.1 資源 registry

native connection は Runtime 所有 registry に置く。VM は型付き token を参照する。

- token は論理 ID と世代を持ち、closed / stale / wrong kind を検査する。ID 再利用で古い handle が別接続へ化けない。
- resource は affine な所有値。無条件の Copy / Share / freeze を許さない。task への移動可否は adapter ごとに宣言する。
- close は明示 API と scope cleanup で確実に処理する。二度目の close は規定した結果を返す。GC finalizer に正常な transaction commit を任せない。
- snapshot に token が残っても閉じた資源は復活させない。recorded 結果を返すだけの replay 用論理 token と実接続を区別する。
- live な owner の scope 終了、task の終了・cancel、Runtime の終了で資源を解放する。checkpoint の token だけが不要な OS connection を永久保持しない。
- 処理中の cancel は worker 完了・解放まで registry に残す。borrow が有効な間は close / 移動を拒否する。
- 再生した close と live close を混ぜる場合、host の二重 close を起こさない。閉鎖後の記録再利用と fresh 操作の失効を別ケースとしてテストする。

### 3.2 host I/O と scheduler

bounded worker pool または適切な Rust 非同期 I/O を用い、VM / GUI thread を長い通信や query で停止させない。OS thread が VM heap を直接読み書きしない。

1. VM 側で引数を検証し、必要な immutable buffer を移すか共有する。
2. 操作を registry に登録して外部へ発行する。
3. VM task は待機状態になる。主 UI task と他の ready task を進める。
4. worker 完了を Runtime が受け取り、結果を記録してから VM の値へ変換する。
5. cancel / timeout / revert 中の完了も同じ操作 ID に結び付ける。

worker 数、queue 長、in-flight byte、接続数を制限する。await / yield に公平性を持たせ、純粋な重い計算にも協調 safe point を用意する。native kernel は必要に応じて chunk 単位で cancel を確認する。replay の完了順・配信順を記録し、実時間を使って競合結果を再決定しない。

## 4. v1.5 の実装・受入

[個別草案](REWIND_v1.5.md) に詳述する。最初の段階は外部 region、操作記録、境界検査、時刻観測、schema と資源 token の共通契約。

受入条件は、同一操作が一回だけ host を呼ぶこと、要求の不一致で host を呼ばないこと、fresh が新規操作になること、replay が host を一度も呼ばないこと。記録予約失敗と結果不明、helper / closure 経由の禁止操作、begin / branch / resume、秘密値の除外も必須。HTTP / DB はここで実装済みと扱わない。

## 5. v1.6 ネットワーク client

### 5.1 API 案

`std.http` に Request / Response / Header / HttpError を置く。

- method、URL、複数値 header、Bytes body、timeout、response byte 上限を型付きで渡す。
- Response は status、header、bounded Bytes body。UTF-8 / JSON は別の pure decoder で扱う。4xx / 5xx は HTTP response であり、自動的に接続エラーへ変換しない。
- query / path encoding、重複 header、大文字小文字、非 UTF-8 body を扱う。ログに credential を含む URL を出さない。
- Rust の成熟した HTTP / TLS 実装を使い、証明書・hostname 検証を既定で有効にする。shell の curl 起動で代替しない。
- system root と明示 CA の設定を定義する。全証明書を無検証で受理する API を便利な既定にしない。
- redirect は既定で無効、または有限回数を明示する。method の変更、別 origin への認証転送、追加送信の記録を定義してから有効化する。
- proxy、connection reuse、connect / total / read deadline を扱う。decompression を使う場合は展開後 byte 上限も守る。
- download は bounded chunk stream を追加し、巨大 body の一括 VM allocation を避ける。upload の消費・記録方針も対称に定義する。

### 5.2 検証

ローカル HTTP / TLS server で GET / POST、binary、duplicate header、redirect、遅延、切断、byte limit、不正証明書を試す。request 数を server 側で数え、revert / resume / replay が追加送信しないことを検証する。

GUI が resize / cancel / close に応答する間に request を待てること、キャンセル後に worker・buffer・接続が残らないことを両 OS で確認する。外部依存がない fixture と実 adapter の試験を併用し、fixture だけで接続成功と扱わない。

## 6. v1.7 DB

### 6.1 接続と共通型

SQLite を self-contained な最初の adapter とし、PostgreSQL で別プロセスの DB への実接続・TLS・credential・timeout を確認する。前者だけで DB 接続全体を完了としない。

- `std.db` に Connection / Statement / Row / DbValue / DbError。null、Bool、Int、Float、String、Bytes を扱い、Decimal / datetime は 1.8 で追加する。
- SQL と parameter を分け、placeholder は adapter に合わせて明示する。文字列結合で parameter を SQL 化しない。
- execute は affected rows、query は bounded batch / cursor を返す。row count・byte・column 上限と終端を定義する。結果を全件常時 List 化しない。
- column name / index、duplicate name、SQL null、型変換失敗は規定した Result。error は SQLSTATE 等の公開 code を持ち、credential / parameter / host の秘密を表示しない。
- prepared statement を接続 lifetime に結び付ける。close 後、transaction 終了後の cursor の扱いを明示する。
- SQLite path は既存 root / path 制約に従う。接続 adapter の TLS・接続先・認証は明示設定する。

### 6.2 transaction

`beginTransaction` / `commitTransaction` / `rollbackTransaction` と VM の commit / revert を別 API にする。transaction を region の scope 終了で勝手に commit しない。

- VM revert で確定済み DB 書込みを取り消さない。
- transaction 内の execute を再利用するときも、DB transaction の実際の状態を確認する。rollback 済みなのに過去の成功を「現在も未確定の変更がある」と扱わない。
- default は同じ Connection の直列利用。savepoint / isolation / pool は基本契約を保って必要な機能から加える。
- cancel、通信断、commit 応答欠落は部分適用 / 結果不明として報告する。driver の自動 reconnect / retry で write を再発行しない。
- scope 終了は未確定 transaction の rollback を試みて close する。rollback に失敗した場合は cause を残し、成功を保証しない。

### 6.3 検証

SQLite の実 file と PostgreSQL の一時 server で schema、parameter、null / binary、複数 row、rollback、commit、constraint error、disconnect、deadline、closed handle を試す。server は CI の試験設備であり、言語のプロセス実行 API を要求しない。

commit 後に VM を戻しても DB 内容は保持されること、同一操作の再利用で行が重複しないこと、replay で DB server がなくても記録済み計算が成立することを確認する。GUI で query を cancel / close する資源解放も測る。

## 7. v1.8 数値・データ

### 7.1 dense storage

既存 `std.matrix` は checked Int 行列。これを浮動小数点配列の実装済み根拠にせず、別の typed dense storage を追加する。

- Float64 と Int64 の連続 buffer、shape / stride、checked index、slice / reshape、copy / view、明示 broadcast を扱う。
- payload は `Arc` と適切な COW / page により共有する。checkpoint を作るたびに全 buffer を複製しない。mutation は変更領域をコピーし、古い view / snapshot を壊さない。
- native 値の storage ID と VM HeapRef を分け、GC・budget・serde・Send / Share に対応する。生 pointer を VM の整数として公開しない。
- 重なる slice の代入は alias 検査か安全な一時領域で処理し、不正な同時 mutable borrow を作らない。
- kernel は長さ / shape / 費用を先に検証し、Rust loop で実行する。要素ごとの Value box、JSON 往復、callback dispatch を避ける。
- contiguous / strided の費用を API に記載し、暗黙の巨大 copy を避ける。bitwise な完全一致を要求する操作と誤差を許す演算を区別する。

### 7.2 数学・線形代数・統計

有限性、NaN / Inf、overflow / domain error の扱いを定義する。

- exp / log / sqrt / 三角関数、round、stable sum、乱数分布。乱数は既存の checkpointed generator を利用する。
- vector 演算、dot、matrix product、transpose、LU / QR、linear solve、least squares、基本固有値計算。solve を逆行列の生成で代用しない。
- mean / variance / covariance、quantile、histogram、correlation、欠損値の方針。online 集計と数値的に安定した演算を使う。
- sparse CSR の構築・matvec と反復 solver、FFT / convolution は 2.0 の数値処理範囲として設計し、dense 実装との精度・費用を比較する。
- random sampling と最適化の seed / convergence / 最大反復 / stopping criterion を記録する。

参照結果と residual、絶対 / 相対誤差、条件の悪い入力、空・極端な shape、alias、revert 前後の値を検証する。精度・正しさを優先し、benchmark に根拠のない高速化を入れない。

### 7.3 標準型と text / stream

既存 API の仕様を維持し、新しい範囲は独立した API で足す。

- BigInt：parse / format、比較、四則、div / remainder、bit operation。既存 checked Int の意味を変えない。
- Decimal：精度・scale・rounding を明示し、金額等の値を Float 経由で変換しない。DB と JSON の表現を定義する。
- 日時：Instant / Duration、UTC と offset、calendar、parse / format、IANA zone と DST の重複・欠落時刻。time の観測と pure な変換を分ける。
- Unicode：既存 scalar API に加え grapheme、正規化、大小変換。GUI の編集単位を変更するときは既存 cursor の移行を設計する。
- regex：bounded compile / match、Unicode と byte の区別、captures。無制限 backtracking を標準の既定にしない。
- streaming JSON / CSV：chunk をまたぐ UTF-8、token、行・構文状態、上限、エラー位置、cancel。巨大 file を一括読込み前提にしない。

各型の generic、Map key、Serde、Result、所有権、freeze、task 移動、API snapshot、LSP を組合せテストする。どの型も名前を追加するだけで完了としない。

## 8. v1.9 GC・性能・GUI・言語監査

### 8.1 現在の memory model

現在は persistent heap、paged List、persistent Map と Arc による共有がある。VM は allocation 数に応じた safe point の mark / sweep を持ち、循環も対象とする。通常の mark / sweep は current heap を掃除し、checkpoint が持つ独立 heap root は保つ。

従って、現在の checkpoint が到達不能 object を含む heap map 自体を保存した場合、current heap の回収だけでその古い root の object がすべて消えるとは限らない。到達可能な履歴データと、heap map に残った死んだ entry を区別して監査する。

### 8.2 必須の監査と改善

- GC root：globals、operand stack、frame、closure の cell、defer、branch、cold / waiting / completed task、channel queue、native callback 引数・返却中の一時値。
- snapshot root：VM が保存した stack / frame / scheduler と Runtime heap の対応を検査する。embedding では caller が持つ Value を明示 root として渡す契約を維持する。
- checkpoint 時の dead entry 保持を測定し、必要なら snapshot 向け到達集合を確定してから root を保存する。checkpoint ごとの無条件 full GC にしない。
- 同じ heap ID を持つ異なる版の object を混同しない。複数 root を扱う GC は root / version の識別が必要。
- trace / spill / journal / completed task / resource registry も retained memory に含める。checkpoint の root と無関係な無限 cache を作らない。
- GC の mark を予算不足で中断したときは sweep しない。成功するまで保持した root の意味を変えない。
- 256 allocation ごとの現行 trigger を、大きい buffer・live byte・前回回収率を測って調整する。固定回数だけで full live heap を何度も走査しない。
- Arc の参照カウントは immutable storage の寿命に利用し、VM の循環回収は tracing を維持する。GC finalizer で通信・DB commit を行わない。
- work budget と物理 memory admission を分ける。native temporary、COW、queue、compiler / trace の費用を見落とさない。

### 8.3 測定の設計

同じ機械・release build・入力・seed で基準を保存し、warm-up 後の複数回の中央値を比較する。実行時間、CPU 時間、ピーク RSS、live heap / buffer byte、snapshot 保持量、allocation / copy byte、GC 回数 / 時間、接続数を記録する。

| 負荷 | 検証する性質 |
| --- | --- |
| checkpoint なしで短命 collection を連続生成 | 回収後の live byte が頭打ちになり、処理回数に比例した leak がない |
| closure / cell / collection の循環を生成して破棄 | GC 後に到達不能 cycle が残らない |
| 同じ巨大 buffer の checkpoint を増やす | 無変更 root の payload が版数に比例して複製されない |
| 小領域を変更して checkpoint を保持 / drop | 差分費用が説明でき、drop 後に共有 storage が解放される |
| sort / Map / graph / parser をサイズ違いで実行 | 宣言した計算量と実測の傾向が一致する |
| dense product / solve / training iteration | 要素ごとの VM heap 増幅がなく、working set が安定する |
| request / query / cancel を繰り返す | journal・queue・thread・connection の上限と解放が機能する |
| GUI を操作しながら計算・通信 | UI dispatch を計算 / host 待機で無期限に止めない |

受入時に baseline の実測値を添える。解放済みの allocator page が OS にすぐ戻らない場合もあるため、RSS の低下だけで GC 成功を判定しない。無履歴時は live byte の増加傾向ゼロを確認する。性能変更は正しさ・memory・速度を同時に比較し、再現した重大な退行があれば原因を解消する。

### 8.4 GUI

v1.4 の View と scene、入力記録、publish を土台に、複数 window ID、focus、clipboard、file dialog、menu、基本 form / table、スクロールを追加する。文字編集は grapheme / IME と整合させ、クリックによる caret / selection も扱う。

clipboard 読取りや dialog 応答は外部観測、clipboard 書込みは外部作用として明示する。OS の dialog や composition を checkpoint で復元しようとしない。window close 後の参照を診断する。UI thread 制約と native resource registry を一致させる。keyboard navigation、focus、読み上げ用の役割・名前・状態を定義し、OS の accessibility 連携も canvas の見た目だけとは分けて検証する。

GUI + HTTP / DB の例で loading / cancel / error / retry を実装する。revert で View は戻っても確定済み DB は消えないことを表示ロジックでも扱う。OS font / input service、Linux X11 と Windows の両方を試験する。未提供 OS の対応を暗黙に約束しない。

### 8.5 言語・アルゴリズム

新型の generic 推論、associated type、borrow / move と Result、callback / iterator、pattern、module visibility、task の組合せを監査する。既存機能が使えない具体例を回帰テストにしてから、必要な compiler pass を修正する。

具体的な監査例として、`Result<Option<List<Int>>,StdError>` の `Err(_)` / `Ok(None)` / `Ok(Some(_))` による完全な nested pattern が false positive の non-exhaustive 診断になることを修正対象にする。guard と wildcard、enum / tuple の積、所有権と generic substitution を含む pattern space で coverage を検証し、未網羅を見落とさず完全な分岐を受理する回帰を追加する。

既存の sort / graph / DSU / bitset 等を再実装しない。未提供の標準実装は、次の API と契約まで具体化して追加する。既に実装された場合はその契約と性能を検証して利用する。

| 追加する範囲 | 契約・確認 |
| --- | --- |
| lazy segment | associative aggregate / tag composition / identity、半開区間、順序保持、更新・query の対数演算、snapshot |
| flow | Dinic、非負 Int capacity、residual graph、maximum flow / cut、overflow、独立した参照実装との小規模比較 |
| bipartite matching | Hopcroft–Karp、左右 vertex の区別、対応・未対応、重複辺、最大性の参照比較 |
| trie | Unicode scalar / byte を API で区別し、prefix、exact lookup、削除、node 上限と寿命 |
| suffix | byte suffix array / LCP、空入力、重複、O(n log n) を目標とする構築、pure な検索 |
| geometry | vector / orientation / segment intersection / convex hull、整数 predicate の広い中間演算、Float の tolerance を明示 |

大きい入力を処理する容量・work の明示設定と overflow 契約を整える。従来の List / graph / scanner 等の固定上限を一律撤廃せず、size 指定 factory と Runtime の総予算を分ける。stream 処理が任意に大きい入力全体を heap へ保持しないことを確認する。

期待型の適用範囲、型付き collection の factory / callback、fallible iterator、generic error、native resource の借用を優先して組合せを確認する。正しいプログラムが拒否されるケースは最小例と診断を記録し、どの pass を変更するか確定する。再帰や generic 展開を無制限にすることと、通常のプログラムに必要な設定可能上限を提供することを区別する。

Int overflow の既定、型安全、borrow 制約、有限予算を単純に無効化して速度を得ない。native kernel、buffer、VM dispatch、property / generic resolution の重複費用を profile で特定して改善する。

## 9. v2.0 統合・数値拡張・公開

### 9.1 残る基盤

- HTTP server：bounded request、router、response、timeout、shutdown / cancel、backpressure。listen / accept / response 送信は外部作用で、VM revert を server transaction に見せない。
- TCP：接続、bounded read / write、deadline、half-close / EOF、partial write。TLS は成熟した実装を利用する。外部 socket を無制限 byte stream として VM へ流さない。
- 数値：sparse / FFT、最適化、勾配と損失。reverse-mode の tape は dense buffer と演算 node を共有し、optimizer 更新や checkpoint の COW と整合させる。
- gradient は finite difference と照合し、linear model と小さい多層モデルの forward / backward、SGD / Adam、mini-batch、seed、loss 推移、weight 保存 / 読込みを検証する。
- 数値 model の serial format は型 / shape / schema を検査し、任意の実行コードを deserialization で起動しない。

### 9.2 必須の統合サンプル

| サンプル | 受入ケース |
| --- | --- |
| 複数画面のデータ編集 | SQLite / PostgreSQL の読取りと保存、parameter、View の Undo、公開済み DB の保持、閉鎖・失敗 |
| HTTP データ取得と表示 | 日本語 / binary / JSON、deadline、cancel、loading、fresh、記録 replay と source-free |
| HTTP server と DB | 同時要求の上限、parameter、transaction、partial failure、shutdown、資源の解放 |
| 数値解析 | 増分 CSV、統計、least squares / sparse solve、誤差と収束、保存・再読込み |
| 勾配計算 | native dense 演算、gradient check、反復最適化、snapshot / drop、memory 安定 |
| 大きい collection | stream input、sort / graph / range / string 処理、容量設定、work / memory 測定 |

サンプルは REWIND で動くプログラムとして SDK に含める。外部 server なしの fixture と実接続試験を区別する。replay は接続なしで成立し、live 実行では実際に両 DB / HTTP / GUI を使う。

### 9.3 互換性・文書・公開の判定

- 新しい API は effect / ownership / error / cost を API snapshot に含める。既存 API の破壊は移行を明示し、黙って意味を変えない。
- artifact / cache / observation schema は版管理し、互換でない記録は明確な診断で拒否する。古い記録の live 再送による「互換処理」を禁止する。
- native dependency の版・license・platform 前提を固定し、Windows / Linux の SDK へ必要な資材を同梱する。利用者に Rust や別の言語 runtime を要求しない。
- `rewind run` / `rewind compile` / `rewindc`、LSP、help、reference とすべての新例を更新する。
- 回帰・std 契約・両 DB・TLS・native GUI・GC / memory・benchmark・source-free・署名 / checksum・展開後検証を release candidate に対して実行する。
- 未実装・未検証の必須項目があれば 2.0 を公開せず、対応する 1.x patch に戻す。到達判定は [ロードマップ](ROADMAP_v2.md) の全項目を evidence に結び付けて記録する。

## 10. 実装前に確定する判断

以下は忘れてよい項目ではなく、依存する版の実装開始時に決定して契約を更新する。

| 判断 | 決定する時点 | 判断材料 |
| --- | --- | --- |
| external の制御フロー・非同期 region 表現 | 1.5 / 1.6 | unwinding、task context、既存 cleanup / scheduler |
| 操作 ID、fingerprint、秘密の照合、trace schema | 1.5 | branch / task / replay、redaction、記録予約の失敗 |
| HTTP / TLS / DB の Rust dependency | 1.6 / 1.7 | 両 OS、TLS 検証、retry 制御、license、buffer / async 費用 |
| adapter 間の query / 型・transaction 差分 | 1.7 | SQLite / PostgreSQL の実挙動、曖昧な抽象化を避ける |
| dense COW page / contiguous kernel / view | 1.8 | snapshot mutation の費用、vector / matrix 実測 |
| 数値 backend・autodiff tape・serialized model | 1.8 / 2.0 | 精度、size、gradient、SDK の依存と配布費用 |
| journal prefix の回収、GC trigger、実行予算既定 | 1.9 | live root と保持履歴、無履歴負荷、UI / native benchmark |
| GUI widget / OS backend の追加順 | 1.9 | 統合サンプル、native 試験、既存 UI thread の制約 |

定数や dependency の候補を設計だけで実測済みにしない。決定結果は各版の仕様・API・テストへ反映し、この文書も実装と合わせて更新する。
