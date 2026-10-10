# REWINDの自由構想集

更新日：2026-10-10。**実装の約束ではなく、あったら使いたい機能・体験のアイデア集**です。実現難度や工数で先に絞らず、言語の将来像を広く考えます。現在449項目あります。

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
| [実験的な言語・計算モデル](14-experimental-computation.md) | 16 |
| [simulation・game・roboticsのmodel](15-simulation-and-worlds.md) | 16 |
| [プログラムから扱うcheckpointと複数版の値](16-programmable-checkpoints.md) | 9 |
| [巻き戻しを使う関数・task・入力の契約](17-state-contracts.md) | 8 |
| [時間と版を持つ値](18-temporal-values.md) | 12 |
| [永続collectionをlibraryとして組み立てる](19-persistent-collections.md) | 12 |
| [branchを使う探索と計算の再利用](20-branch-search.md) | 12 |
| [外部作用を値として組み立てる](21-effect-values.md) | 12 |
| [所有権・領域・値の費用](22-ownership-and-regions.md) | 12 |
| [式・pattern・日常の書き方](23-expression-design.md) | 12 |
| [型の合成・抽象化・polymorphism](24-type-composition.md) | 12 |
| [streamとeventの合成](25-stream-algebra.md) | 12 |
| [版を跨ぐ計算cacheと再計算](26-versioned-computation-cache.md) | 12 |
| [module・生成code・linkの表現](27-modules-and-linking.md) | 12 |
| [処理系backendと意味を保つ最適化](28-runtime-backends.md) | 12 |
| [snapshotの保存形式・共有・復元](29-snapshot-formats.md) | 12 |
| [protocol・資源の状態を表す型](30-protocol-types.md) | 12 |
| [深い値の操作と双方向の変換](31-bidirectional-data.md) | 12 |
| [数値計算の精度・形・結果の契約](32-numeric-contracts.md) | 12 |
| [確率model・推論・実験の構成](33-probabilistic-models.md) | 12 |

## 読み方

各項目は「どう使えたら嬉しいか」を中心に記載します。仮の用語や操作は現行syntax/APIではありません。既存の58候補と重なる部分は、その先の使い方を考えるための材料です。

VM内部の値・計算・未公開作用には巻き戻しを活用できます。確定済みの外部作用は記録・照合・補償等の別手段で扱う前提を維持します。面白い構想を考えるために、この境界を曖昧にはしません。

## 組合せるとどんな体験になるか

単独の機能だけでなく、複数の構想をつなぐと新しい使い方が生まれます。

| 体験の案 | 組合せる構想 |
|---|---|
| 原因を説明するUndo付きeditor | [時間軸](02-time-and-debugging.md)のVTIME-01/08と[GUI](03-gui-and-media.md)のVGUI-07/10 |
| 数式からsimulationまで辿る文書 | [科学計算](05-science-and-mathematics.md)のVSCI-04/05と[notebook](10-development-and-ecosystem.md)のVDEV-01/02 |
| データの由来を示す分析画面 | [データ](04-data-and-knowledge.md)のVDATA-01/05と[GUI](03-gui-and-media.md)のVGUI-11 |
| 失敗から自動で作る小さなreproducer | [時間軸](02-time-and-debugging.md)のVTIME-12と[検証](09-verification-and-security.md)のVQUAL-02/04 |
| 仮説を比較するmodel開発環境 | [AI](06-ai-and-models.md)のVAI-05/16と[時間軸](02-time-and-debugging.md)のVTIME-05/06 |
| protocolの型・実行・記録を一緒に見る | [実験言語](14-experimental-computation.md)のVEXP-03と[通信](07-network-and-distribution.md)のVNET-16 |
| 暦・金額・根拠を保つreport | [domain library](12-domain-libraries.md)のVDOMAIN-01/05/10/15と[データ](04-data-and-knowledge.md)のVDATA-06 |
| simulationと実測を比べるdigital twin | [world](15-simulation-and-worlds.md)のVWORLD-14/15と[科学計算](05-science-and-mathematics.md)のVSCI-13 |
| 利用者ごとに操作方法を変える同じ画面 | [interaction](13-text-and-interaction.md)のVHUMAN-06/13/16と[GUI](03-gui-and-media.md)のVGUI-16 |
| 異なる実行方式でも検証できる計算library | [systems](08-systems-and-hardware.md)のVSYS-09/14と[検証](09-verification-and-security.md)のVQUAL-06 |
| 型からAPI・文書・testを育てる開発環境 | [言語](01-language.md)のVLANG-06/15と[開発](10-development-and-ecosystem.md)のVDEV-04/06 |
| 大きな入力をmemory階層に合わせてquery | [データ](04-data-and-knowledge.md)のVDATA-02と[systems](08-systems-and-hardware.md)のVSYS-11/13 |

この表は独立した追加項目として数えず、449項目の組合せ例です。採用を検討するときに、短い利用シナリオを作る出発点にできます。

## 仮のcodeで考える利用体験

[構想を組み合わせたsketch](sketches/README.md)では、未実装APIを仮のcodeにして、状態・履歴・外部作用の分担まで掘り下げます。sketchは独立した追加機能として件数へ重ねて数えません。

## 次に選ぶとき

気になる項目を組み合わせて短い利用シナリオを作り、現行APIでできる部分と追加する差分を後から調べます。今の段階では、優先順位・版番号・期限・受入条件を全項目へ付けません。

計画の保存はcodex/developのみ。公開CI/CD、main統合、Release、版更新は行いません。
