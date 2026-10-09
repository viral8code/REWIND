# v2.0 到達条件の残件

v1.9.61 の実装を基準にした作業整理。これは v2.0 の完了宣言ではない。
公開の判定は、実装、実 adapter での検証、Linux / Windows の配布検証を区別する。
到達条件の原文は [ロードマップ](ROADMAP_v2.md)、設計は [詳細設計](v2-design.md) にある。
過去の各版にある「残る作業」の記述はその版の時点の情報であり、現在の不足一覧として再利用しない。

## すでに提供している基盤

HTTP / HTTPS client、bounded HTTP server と TLS、TCP / TLS、SQLite と PostgreSQL、
外部操作の記録付き / live 実行、native window、フォーム・表・編集・clipboard、
dense / sparse 数値型、線形代数・FFT・統計、自動微分・最適化・モデル保存、
既存の collection / graph / range / string アルゴリズムは実装済み。
GC、共有 storage、履歴と native resource の会計にも実装と個別の検証がある。
これらを未実装として再計画しない。

最近の追加は、協調的 backward（1.9.58）、メニュー（1.9.59）、
ファイルダイアログと SDK の streaming hash（1.9.60）。
各版の契約と検証記録を参照し、実装済みと配布検証済みを混同しない。

1.9.61ではIMEのnative preeditと入力コンテキストの寿命を実装し、Linuxの実IBus/Anthyで変換途中・確定・変換・フォーカス移動を検証する。Windowsのcontext/queue試験とUnicode確定入力は、実際の日本語変換エンジンの検証とは区別する。OS accessibilityとフォントサービスの確認は継続する。GUI・HTTP・両DBの8サイクル反復検証も追加するが、数値計算を同時に走らせた長時間安定性の完了には扱わない。

## 必須の残件と完了判定

| 作業単位 | 具体的な変更・検証 | 完了判定 |
| --- | --- | --- |
| IME と accessibility | `src/gui/windows.rs` / `x11.rs` に native composition、caret / focus との連携を整える。OS の読み上げ向け役割・名前・状態の adapter を加える。未確定の native 入力状態を VM snapshot と分離する。 | 両 OS の実入力 service / accessibility client で composition、確定、キャンセル、focus 変更、window close を確認する。mock と Unicode 確定入力だけで完了にしない。recorded input の source-free replay は native service を必要としない。 |
| 計算と応答性 | 自動微分の forward tape 構築、初期入力のコピー・key / digest 作成など、まだ長い同期処理が残る経路を profile する。必要な経路を分割し、同期 API の既存契約を保つ。 | 非ゼロの materialized model で forward / backward の正しさ、計算中の native GUI 入力、キャンセル、保持された checkpoint と最後の参照の解放を確認する。同じ入力の同期版と CPU / wall / RSS / live storage を比較し、追加費用を明記する。 |
| 統合反復負荷 | GUI・HTTP server・両 DB の transaction / partial failure / shutdown を同じ VM で反復する。さらに通信・数値計算の同時負荷、切断・キャンセル・終了経路を組み合わせる。 | 確定済み DB を独立に照合し、revert / replay が書込みを再実行しない。thread / connection / queue / temporary storage の枠と終了時の解放を確認する。意図的に保持する観測・checkpoint と到達不能な一時値を区別して、反復数に対する増加を測る。 |
| 最終受入・配布 | 到達条件1〜7を具体的な実装・試験・測定へ結び付ける。診断・reference・API / effect / ownership / cost・native dependency / license・SDK を照合する。 | release candidate の Linux / Windows で全回帰、実 DB / TLS / GUI、GC / memory / benchmark、展開後 source-free、署名 / checksum を確認する。必須項目の未実装・未検証があれば v2.0 を公開しない。 |

## 統合検証で確認した制限

- debug 記録には step inspection 用の固定 16 MiB 上限がある。長い反復試験は compact
  記録を使い、短い debug の検証も残す。上限による明示的停止を leak や接続失敗と混同しない。
- PostgreSQL の接続開始には 192 MiB の保守的な admission 予約がある。GUI などの同時利用では
  追加の容量が必要。予約量は実際に同時使用している RSS と同じ値ではない。
- 協調実行には scheduler / step の追加費用がある。分割しただけで高速化・省メモリ化したと
  扱わない。既存の同期 API と実測の比較を残す。
- 個別 kernel の回収試験と、複数 adapter を使い続けたシステム全体の安定性は別の検証とする。

## 作業と公開の単位

修正一つごとに公開版を増やさず、上の作業単位に必要な変更と検証をまとめる。
検証中に見つかった同じ範囲の修正は、その未公開版へ含める。
各公開版の両 OS 検証、署名付き Release、main への統合は維持する。
新しい不足を見つけた場合は、この一覧の該当行に具体例と完了判定を追加する。
版番号やテスト数の増加を v2.0 の到達判定に使わない。
