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
| 1.7.0 | SQLite、PostgreSQL adapter、parameter、行 / cursor、独立した DB transaction | 両 DB の実接続、部分失敗、確定済み書込みと VM revert の独立、解放・replay |
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
