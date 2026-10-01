# REWIND v1.6.0 草案

v1.5 の外部操作記録を土台に HTTP / HTTPS client と host I/O の非同期待機を追加する。[詳細設計](v2-design.md) のネットワーク、scheduler、資源 lifetime を共通契約とする。

## 段階

1. 型付き Request / Response / Header / HttpError、method / URL / binary body、header の複数値、明示 byte / deadline 制限を定義する。
2. 成熟した Rust HTTP / TLS backend を使い、hostname / certificate 検証、system root / 明示 CA、proxy、connection reuse を整える。自動 retry と暗黙の認証転送は禁止する。
3. 操作 ID と worker の物理 ID を分離し、bounded host worker / queue、task ごとの external context、完了順記録を実装する。待機中も GUI と他の task を進める。
4. timeout / cancel / 通信断を未適用・部分適用・結果不明から区別する。revert 時に同じ in-flight request を再発行しない。
5. bounded chunk download / upload、UTF-8 / JSON の pure decoder と統合する。圧縮する場合は展開後の容量も制限する。

## 受入

実 HTTP / TLS server で GET / POST、binary、header、redirect の明示制御、deadline、切断、body limit、不正証明書を確認する。server 側の request 数で再送防止を確認し、replay では server がなくても記録済み計算が成立することを検証する。

GUI の操作・close / cancel が request 待機で停止しないこと、終了後に worker / buffer / connection が残らないことを両 OS で測定する。権限、秘密、API snapshot、source-free、SDK 配布も同じ版で揃える。
