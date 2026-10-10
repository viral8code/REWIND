# REWINDの自由構想集

更新日：2026-10-10。**実装の約束ではなく、あったら使いたい機能・体験のアイデア集**です。実現難度や工数で先に絞らず、言語の将来像を広く考えます。現在208項目あります。

[既定の開発計画](../ROADMAP_v2-next.md)と[58候補の選定表](../v2-next-selection.md)は、これまでどおり別に維持します。この構想集から選んだものだけ、後で具体的な設計・実装計画へ移します。既存機能があるものは発展形・統合体験の構想であり、現行版に機能がないという主張ではありません。

## 分野別の入口

| 分野 | 構想数 |
|---|---|
| [言語表現とメタプログラミング](01-language.md) | 16 |
| [時間・分岐・デバッグの新しい体験](02-time-and-debugging.md) | 16 |
| [GUI・描画・文書・メディア](03-gui-and-media.md) | 16 |
| [データ処理・検索・知識の構造](04-data-and-knowledge.md) | 16 |
| [数理・科学計算・simulation](05-science-and-mathematics.md) | 16 |
| [AI・model・学習・評価](06-ai-and-models.md) | 16 |
| [通信・分散・外部連携](07-network-and-distribution.md) | 16 |
| [OS・hardware・storage・実行基盤](08-systems-and-hardware.md) | 16 |
| [検証・故障探索・情報の境界](09-verification-and-security.md) | 16 |
| [開発環境・配布・言語間連携](10-development-and-ecosystem.md) | 16 |
| [高度なalgorithmとデータ構造](11-algorithms-and-structures.md) | 16 |
| [金額・暦・文書・日常データのlibrary](12-domain-libraries.md) | 16 |
| [文字・入力・言語・accessibility](13-text-and-interaction.md) | 16 |

## 読み方

各項目は「どう使えたら嬉しいか」を中心に記載します。仮の用語や操作は現行syntax/APIではありません。既存の58候補と重なる部分は、その先の使い方を考えるための材料です。

VM内部の値・計算・未公開作用には巻き戻しを活用できます。確定済みの外部作用は記録・照合・補償等の別手段で扱う前提を維持します。面白い構想を考えるために、この境界を曖昧にはしません。

## 次に選ぶとき

気になる項目を組み合わせて短い利用シナリオを作り、現行APIでできる部分と追加する差分を後から調べます。今の段階では、優先順位・版番号・期限・受入条件を全項目へ付けません。

計画の保存はcodex/developのみ。公開CI/CD、main統合、Release、版更新は行いません。
