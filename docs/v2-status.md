# v2.0 到達条件の残件

v1.9.64までのWindows / Linux公開版と、v1.9.65の検証中候補、v2.0候補を基準にした作業整理。これは v2.0 の完了宣言ではない。
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
ファイルダイアログと SDK の streaming hash（1.9.60）、native IME（1.9.61）、
協調的forward（1.9.62）、OS accessibilityとfont検証（1.9.63）。
各版の契約と検証記録を参照し、実装済みと配布検証済みを混同しない。

1.9.61ではIMEのnative preeditと入力コンテキストの寿命を実装し、Linuxの実IBus/Anthyで変換途中・確定・変換・フォーカス移動を検証する。Windowsのcontext/queue試験とUnicode確定入力は、実際の日本語変換エンジンの検証とは区別する。OS accessibilityとフォントサービスの確認は継続する。GUI・HTTP・両DBの8サイクル反復検証も追加するが、数値計算を同時に走らせた長時間安定性の完了には扱わない。

1.9.62の公開版では、既存演算すべての分割実行と同期版とのNode／値／勾配一致、保存領域の分割identity準備、途中のcheckpoint復元・取消・source-free replayを確認している。64／256回の取消でGCとlive数値ページの上限を確認し、Linuxのnative pointerによる計算中の取消も検証する。全回帰・両OSの展開後SDKも通過した。最適化測定では協調実行に追加費用があり、初期入力や長時間統合負荷の完了へこの結果を広げない。

1.9.63の公開版では、Linuxの実AT-SPI registry / system clientで名前・役割・状態・Unicode Text・既定操作・変更通知を確認した。source-freeのdebug / compact実行でnative操作によるVM復元とサービスなしreplayを確認し、実X11 captureでLatin / 日本語の異なる字形を確認した。Windowsの実COM clientとfont capture、両OSの展開後SDKと全回帰も通過した。Windowsの実日本語変換エンジンの確認や数値・通信との長時間負荷を、この結果だけで完了にはしない。

1.9.64の公開版では、所有するListからの分割数値入力、途中復元・取消・回収と、
数値・GUI・HTTP・両DBの同時実行を確認した。両OSの全回帰と展開後SDKでは、
正常終了、切断、早期終了、source-freeのdebug / compact実行、サービスなしreplayを
確認した。Linuxの両DB各16サイクルでは計算完了、確定結果の独立照合と回収も確認した。
長いcompact replayには再生費用があり、応答性のための協調実行も追加費用を伴う。

1.9.65候補には、子taskの診断・型付きbudget保持、大規模collectionのsnapshot / iteratorと
COW、native資源の共通roots走査を含める。ローカルの全650回帰とdoc、標準ライブラリ、
展開後SDKは通過した。公開候補CIのWindows実Microsoft日本語IMEでも、
preedit / convert / commitの8回反復、focus取消、closeを確認した。
句変換モードはサービスの既定を保持する。両OSの最終回帰・SDKと公開の判定は
Release workflowで確認する。v2.0の公開完了を意味しない。

v2.0候補ではnative factoryが返すList / Mapを通常の所有値として変更できるようにする。
rootおよびOption / Result内のcollectionにVM headerを一つ確保し、共有ページを保つ。
数値値・shape・Unicode分割結果の変更、元データとsnapshotの独立性、source-free replay、
下位languageのartifactの挙動維持は重点テストを通過した。Linux候補の全回帰、
標準ライブラリと展開後SDKも通過した。両OSの最終公開CIと公開判定が残っている。

## 必須の残件と完了判定

| 作業単位 | 現在の状態 | 残る完了判定 |
| --- | --- | --- |
| OS入力・accessibility | native IME、実Linux IBus / Anthy、AT-SPI、Windows COM client、両OS font captureは実装・検証済み。 | v1.9.65候補の実Microsoft IMEでpreedit / convert / commit / focus / closeは確認済み。v2.0の最終候補でも同じ実サービスの受入を維持する。mockやUnicode確定入力で代用しない。 |
| 計算・統合反復負荷 | forward / backward / 入力準備の協調実行、値・勾配の一致、実GUIからの取消、両DBとの同時負荷・切断・早期終了・回収を検証済み。 | v2.0の最終候補と両OS SDKで同じ契約の回帰を通す。CPU / wall / RSS / live storageと協調実行の追加費用を照合する。 |
| 型・コレクション・診断 | v1.9.65の大規模snapshotと元診断保持はローカル全回帰済み。v2.0のnative返却collectionの変更は重点試験済み。 | v1.9.65の両OS公開検証と、v2.0の全回帰・標準ライブラリ・source-freeの展開後SDKを通す。旧artifactを新languageへ読み替えない。 |
| 最終受入・配布 | 到達条件1〜7の実装と検証の対応を整理中。v1.9.64は193ライセンス見出しと両OS依存の照合、署名付き配布、main / tag一致を確認済み。 | v2.0候補の診断・reference・API / effect / ownership / cost・依存ライセンスを照合し、両OS全回帰、実DB / TLS / GUI、資源回収、署名 / checksum、公開Release / main / tagの一致を確認する。未検証の必須項目があれば公開しない。 |

## 統合検証で確認した制限

- debug 記録には step inspection 用の固定 16 MiB 上限がある。長い反復試験は compact
  記録を使い、短い debug の検証も残す。上限による明示的停止を leak や接続失敗と混同しない。
- PostgreSQL の接続開始には 192 MiB の保守的な admission 予約がある。GUI などの同時利用では
  追加の容量が必要。予約量は実際に同時使用している RSS と同じ値ではない。
- 協調実行には scheduler / step の追加費用がある。分割しただけで高速化・省メモリ化したと
  扱わない。入力変換の262144要素では分割版に約2.27倍の時間・約1.75倍のpeak RSSを
  要した。既存の同期 API と実測の比較を残す。
- 個別 kernel の回収試験と、複数 adapter を使い続けたシステム全体の安定性は別の検証とする。

## 作業と公開の単位

修正一つごとに公開版を増やさず、上の作業単位に必要な変更と検証をまとめる。
検証中に見つかった同じ範囲の修正は、その未公開版へ含める。
各公開版の両 OS 検証、署名付き Release、main への統合は維持する。
新しい不足を見つけた場合は、この一覧の該当行に具体例と完了判定を追加する。
版番号やテスト数の増加を v2.0 の到達判定に使わない。

数値の協調ライブラリの子task失敗は、v1.9.65候補で具体的なcodeと行へ投影する。
元の診断全文・原因ツリー・型付きbudgetは`std.taskError.describe`で保持する。
`StdError`投影はその全情報を保持しない。この違いをreferenceと回帰で確認する。
