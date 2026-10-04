# REWIND v1.9.21

## 明示的な live 外部操作

language 1.9.21 以降で `external live { ... }` を使える。`live` と `external` に加え、操作に応じて `network` / `db` / `clock` / `tasks` を許可する。region は権限を自動付与しない。通常の `external` / `external fresh` の記録・再送防止・replay は維持する。`live` は文脈上の指定であり、通常の変数名としても使える。

新たに live region の操作を実行すれば、同じ要求でも物理操作を再実行する。region より前へ revert / resume / begin した後の再実行も新しい送信となる。DB 書込み・通信等を取り消す保証はなく、二重適用を避ける責任は呼出し側にある。既存 Task を保持した checkpoint に戻る場合は、その Task 自体を再送せず、進行中の操作または完了結果を再利用する。

Task の待機は region の外で行う。所有権、mutable borrow の await 越境禁止、region の入れ子や checkpoint / task switch の禁止、接続の上限・deadline・キャンセル・typed error は従来の規則を適用する。閉じた接続は revert でも復活しない。

## 結果と接続の寿命

非同期操作の ID は記録付き操作と分離し、VM の復元では再利用しない。live の未完了結果 buffer と metadata を実操作前に予算へ計上する。空の readiness poll を観測 tape に蓄積しない。完了結果は private immutable storage として必要に応じて spill し、公開 trace に含めない。

Task と checkpoint は結果保持用の lease を共有する。最後の保持先が消えると release queue へ登録し、VM scheduler の進行時に解放する。保持済み全 checkpoint を解放のたびに走査しない。放棄した未完了操作は cancel する。結果からまだ取得していない DB / HTTP stream の接続・statement・cursor も解放する。取得済み affine handle は従来の scope / reachability / close 規則で管理する。履歴が保持する既存 Task の結果は保持する。

解放時の native work も予算へ含める。予算不足で解放が中断した際は未処理の queue を維持し、VM 終了時には Runtime の Drop によって物理資源を解放する。予算は revert で回復しない。

`rewind profile` は live を使った場合に `runtime.external_live` を出力する。`retained_operations` / `pending_operations`、`outcome_bytes`、`reservations_and_metadata_bytes`、`native_resources`、既存 recorded 操作・poll 数を確認できる。これは process RSS や全 heap のメモリ量ではない。

## 記録・開発支援

live 実行には完全な観測 tape がない。`--record`（debug / compact）、replay、inspect / pause、観測を保持する REPL は `ExternalLiveRecordingUnsupported` で拒否する。CLI は entry から到達し得る effect を検査し、live を使う経路がある場合は最初の publish や物理操作より前に拒否する。実行されない分岐も effect として扱う。Runtime API も既定では live を許可せず、明示的な設定と実行 domain を要求する。

source compile / source-free `.rwc` は利用できる。従来の `external` によるオフライン replay を必要とする場合は live を選ばない。

## 検証と残る範囲

単体試験は最後の lease の解放、checkpoint の保持、cleanup 予算不足時の queue、実行前の allocation / work 制限を確認する。実 HTTP の要求数、キャンセル、SQLite の書込みと資源寿命、PostgreSQL の TLS 接続を検証する。コンパイル後 source-free 実行でも既存 Task 復元と新規送信を区別する。署名付き Windows / Linux SDK に live の SQLite 例と実 HTTP の検証を含める。

この版は client / DB / clock の live domain を追加する。GUI input の live 観測、HTTP server / TCP、残る GUI 統合、cooperative kernel、一般 container の保持費用・capacity は別途実装する。v2.0 の到達条件を満たしたとは扱わない。
