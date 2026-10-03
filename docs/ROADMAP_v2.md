# REWIND v2.0 までの実装計画

この文書は計画であり、後続版の実装完了を示さない。基準は v1.4.0（`97e9670`）。初期計画を基準に各版の実装を進め、実装内容は各版の仕様と検証記録に反映する。

設計、変更対象、失敗時の扱い、検証条件は [詳細設計](v2-design.md) にまとめる。外部作用の初期設計は [v1.5 草案](REWIND_v1.5.md) を参照する。

## 到達条件

v2.0 は次の条件をすべて確認してから公開する。機能名が存在することだけを完了条件にしない。

1. GUI、ネットワーク、DB を組み合わせたプログラムが、Windows / Linux の SDK でソース実行・コンパイル後実行できる。
2. VM 内の状態、観測記録、公開済み作用、外部接続の寿命が区別され、revert / resume / begin と併用しても外部作用を二重実行しない。外部システムの巻き戻しを保証しない。
3. 型、generic / trait、所有権、エラー、module、タスクと新しいライブラリが組み合わせて使える。既存制限が通常のデータ処理を妨げる箇所は測定と実例に基づいて解消する。
4. 数値配列、線形代数、統計、最適化、勾配計算とモデルの保存・読込みを、セルごとの VM heap object を大量に作らず実行できる。
5. 到達不能な値・循環・一時 buffer・終了した task・閉じた接続を適切に解放する。checkpoint に保持された到達可能データは維持し、履歴を捨てれば共有 storage も解放される。
6. 数値精度、計算量、ピークメモリ、GC 時間、応答性を測定し、無駄なコピー・全履歴走査・繰り返す接続確立などの重大な費用を解消する。
7. 実行時診断、言語リファレンス、API の失敗・所有権・費用契約、サンプル、署名付き配布を実装と一致させる。未確認事項を実装済みとして扱わない。

不足が残る場合は該当する 1.x の patch 版で解消し、版番号の更新をもって到達としない。

## 現在の基盤

| 領域 | v1.4 にあるもの | 追加・確認するもの |
| --- | --- | --- |
| 言語 | 条件分岐、loop、再帰、tuple、record / enum、match、closure、generic / trait、所有権、Option / Result、effect | 新しい型・資源との整合、期待型推論・借用診断・標準型の組合せの不足監査 |
| 巻き戻し | checkpoint / branch、revert / resume、予約 begin、増分 publish、入力記録と replay | HTTP / DB 等の即時外部作用、接続の寿命、操作記録の保持方針 |
| データ | List / Map、永続 heap / paged List / persistent Map、Bytes / String、JSON / CSV / chunk stream | dense 数値配列、増分 parser、必要な大きさを設定できる容量契約 |
| アルゴリズム | sort / search、heap / deque、DSU、Fenwick / segment、graph、文字列探索、bitset、整数行列等 | lazy range、flow / matching、suffix / trie、transform 等の未提供部分を監査して追加 |
| 実行 | async task、channel、TaskGroup、有限 work / history budget、自動 GC | 外部 I/O の待機・キャンセル、公平性、GC trigger と測定、負荷時の UI 応答 |
| GUI | Windows Win32 / Linux X11、基本 widget、Unicode 確定入力、選択編集、配置、polling | 複数 window、clipboard / dialog、フォーム・表、grapheme、非同期 I/O と連携 |
| 外部 I/O | 仮想 file / directory、publish、時刻・環境等の観測 | HTTP / HTTPS、TCP の基礎、SQLite、別プロセスの DB への接続 |
| 開発支援 | CLI、source-free artifact、LSP、JSON 診断、API snapshot、両 OS SDK | 新構文・新型の支援、長時間実行の診断、移行・配布検証 |

既存実装は [言語リファレンス](language-reference.md)、[標準ライブラリ](../libraries/README.md)、[v1.4](REWIND_v1.4.md) を参照する。古い文書の module 数やテスト数は実装時に再集計し、機能の有無を数字だけから判断しない。

## 版ごとの計画

| 版 | 主な変更 | 公開の判定 |
| --- | --- | --- |
| 1.4.0 | GUI 編集・Unicode 確定入力・resize・polling | 公開済み。新規実装の基準 |
| 1.5.0 | 明示的な外部領域、操作 ID / 記録、再実行防止、失敗分類、資源 registry の契約 | 境界の静的・動的検査、同一 / 異なる要求、fresh、記録失敗、begin / branch / task との検証 |
| 1.6.0 | HTTP / HTTPS client、TLS、timeout、bounded body、非同期 host I/O とキャンセル | 実 server との通信、無断再送なし、replay が host に触れない、GUI が応答する |
| 1.6.1 | 逐次 HTTP download / upload、接続の所有権と cleanup | 実 server、容量・期限・秘密値、資源解放、revert / source-free replay |
| 1.7.0 | SQLite adapter、parameter、行 / cursor、prepared statement、独立した DB transaction | 実 DB、部分失敗、確定済み書込みと VM revert の独立、解放・replay |
| 1.7.1 | PostgreSQL adapter、実接続・TLS・認証 | 実 server、SQLSTATE、切断・期限・transaction、両 OS。ここまで通して v1.7 の DB 範囲を検証 |
| 1.8.0 | dense 数値型、数学・線形代数・統計、BigInt / Decimal、日時 / text / stream の不足補完 | 数値精度、サイズ・費用契約、snapshot の共有、実データの増分処理 |
| 1.9.0 | GC / ownership 監査、VM と native kernel の高速化、公平な task、GUI 拡充、残るアルゴリズム | メモリの長時間安定、履歴の保持・解放、性能測定、UI と通信 / DB の並行動作 |
| 2.0.0 | 統合、HTTP server / TCP 基礎、数値・勾配 / 最適化の完成、言語・配布の不足修正 | 到達条件の全項目、全 adapter / OS、source-free、仕様・API・benchmark の整合 |

依存が大きい項目は、主版の中で垂直に切る。例えば 1.7.0 で SQLite、1.7.1 で PostgreSQL を検証する場合、1.7 の DB 範囲は後者まで終わってから完了とする。外部境界や記録形式の不具合は patch 版を優先し、後続の新機能を積み重ねない。

## 共通の設計方針

- VM の値と未公開 I/O は巻き戻す。観測済み結果・外部で確認された書込み・不明な送信結果は消さない。
- 外部作用は即時実行として明示する。通常の publish へ偽装しない。DB transaction の commit / rollback は VM の commit / revert と別物とする。
- 数値 buffer や protocol parser は必要な部分を Rust に置き、上位の便利な API は REWIND の標準 module で提供する。外部の別処理系を起動して置き換えない。
- VM heap の既存 mark / sweep と永続 storage を土台にする。参照カウントだけでは回収できない循環を忘れない。GC と接続 close の責務を分ける。
- 型・効果・所有権・予算・診断・artifact / replay / LSP・SDK は同じ版で揃える。
- 他の大規模機能より先に project 管理を拡張しない。外部プロセス実行は今回の到達条件に含めない。

## 作業・公開

作業 branch は `codex/develop`、公開 branch は `main` とする。版ごとの branch は作らない。

1. 各版の実装前に現状を照合し、必要な API・保証・受入ケースを確定する。
2. 変更を compiler / VM / source std / tooling / examples に反映する。変更する名前・wire format・公開 API には移行方針を添える。
3. 回帰、標準契約、外部 adapter、性能・memory、Linux / Windows SDK を検証する。
4. compiler / language / std / lock と docs を揃え、署名・checksum・展開後実行・source-free を検証して Release 用の成果物を準備する。
5. 両 OS の Release 検証成功後、その tag の commit を `main` に統合してから Release を公開する。後続版の作業は含めない。公開処理の失敗時は同じ tag を再利用し、検証・統合済みであることと公開状態を区別して報告する。

計画の更新と実装は develop で進め、版ごとの検証・main 統合・Release 公開を完了してから、その版を公開済みとして記録する。

## 段階の補足

v1.5.0 は外部領域の基盤を実装済み。v1.6.0 は上限付き HTTP body と Task / GUI の連携を提供する。大容量の逐次 download / upload と接続型資源の共通 lifetime は v1.6.1 で安定化し、v1.7 の DB に進む前に検証する。元の到達条件は維持する。

v1.7.0 は SQLite adapter と DB の基本契約を公開済み。v1.7.1 は PostgreSQL / SCRAM / verified TLS と実 server の source-free replay を追加し、公開前に両 OS の SDK で検証する。後続の数値・GC・GUI・統合の到達条件は維持する。

### 1.8 の公開単位

1.8.0 は native dense FloatArray / IntArray、COW view と更新、scalar math / vector / LU / 基本統計を先に検証する。1.8.1以降で QR / least squares / eigen と追加統計・分布乱数、標準数値型と DB / JSON 変換、日時 / Unicode / regex / 増分 stream を垂直に実装する。全項目を終えるまで1.8工程は完了としない。各 patch も両 OS の SDK を検証し、main 統合と Release 公開を行う。

### 1.9 の公開単位

1.9.0 は nested match / module scope の言語監査と大きい payload に対する GC trigger を最初の単位として検証する。観測 / trace の保持・spill、bounded file I/O、共有 buffer の費用、native kernel の公平性・キャンセル、GUI と残るアルゴリズムの実例・測定は後続 patch で継続する。1.9.0 の公開をもって 1.9 全工程の完了とはしない。

1.9.1 では完了した外部結果を既存 private Segment spill に統合する。wire と再送禁止を保ち、少ない resident memory と disk 上の履歴を区別する。現行 DOM trace の上限は継続し、trace の streaming、file I/O、公平性等の残りを完了扱いにしない。

1.9.2 では snapshot / virtual file の読み出しを page 範囲に限定し、仮想 snapshot を共有する。終了 task の実行用引数・global bindings も解放する。host snapshot の全体 materialization と、task 結果・channel・group の到達可能性監査は継続する。

1.9.3 では current scheduler の task / channel / group の到達可能性を検査し、必要な handle・待機・未観測 failure・checkpoint を維持して回収する。heap root の深い Value clone を借用へ置き換える。累積 quota / profile metadata と他の観測領域まで回収済みとは扱わず、後続監査を続ける。

1.9.4 では contiguous 数値配列の走査を page 単位にし、import 内の nominal generic scope を修正する。初期履歴予算の CLI 指定と replay を整合させる。native kernel の公平性・キャンセルと残る GUI / algorithm / 統合の受入条件は継続する。

1.9.5 は iterative Dinic の最大流と二部マッチング / 最小 vertex cover を標準 module に追加する。VM 内の residual network、長い経路、独立な cut / matching 列挙、source-free replay と SDK を検証する。残る algorithm、GUI、native kernel と履歴の費用は継続する。

1.9.6 は compact 記録により長い純粋計算の debug index 保持を省き、実行順序と観測・最終状態の検証を維持する。state digest の整形・journal 読込みも逐次化する。観測 export の DOM 上限と全領域の保持費用は別に継続する。

1.9.7 は GUI に選択可能な grapheme 編集を追加し、scalar offset / 旧編集との互換、checkpoint と source-free replay を検証する。multi-window、clipboard / dialog、native kernel の公平性と残る統合条件は継続する。

1.9.8 は generic の制約を effect の部分特殊化でも保持し、lazy range、再利用可能な UTF-8 / byte Trie、native suffix / LCP、exact Int geometry を実装する。callback の回数・順序、容量・node 再利用、独立参照と source-free SDK を検証する。matching は現行 flow backend を維持する。native kernel の公平性、GUI 拡充、統合の到達条件は継続する。

1.9.9 は名前付きの複数 Win32 / X11 surface、scoped / any 入力、公開済み画面の保持、入力の事前予算検査を実装する。clipboard / dialog / 表 / accessibility、native kernel の公平性と残る統合は継続する。

1.9.10 は明示的な task handoff、ready task の既定巡回、協調的な dot / matmul、replay の native / execution 予算復元を実装する。solve / QR / eigen 等、数値 page 共有の admission accounting、残る GUI / 通信 / 数値統合は継続する。

1.9.11 は数値 page / tree の Runtime ごとの会計を差分化し、checkpoint と scheduler の重複計上を解消する。弱い registry と COW / Drop により到達可能な storage の費用を保持・解放し、source-free と long-run を検証する。一般の container / metadata の費用、残る GUI / kernel / 通信 / 数値統合は継続する。

1.9.12 は typed schema のフォーム検証、frozen データの表の表示範囲描画、GUI control の編集と状態復元を追加する。clipboard / dialog / menu、IME / accessibility、GUI と通信・DB の長時間統合、追加 kernel と v2.0 の到達条件は継続する。
