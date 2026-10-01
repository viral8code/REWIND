# REWIND v1.6.1 草案

v1.6.0 の上限付き HTTP client を保ち、大容量 transfer と接続型資源の lifetime を DB の前に安定化する。

1. `HttpDownload` / `HttpUpload` を registry の論理 ID で参照する。所有値の無条件 Copy / Share / freeze を拒否し、read / write 中の borrow と close / move を検査する。task への移動は明示的に対応する。
2. headers と body の逐次 read / write を別の記録操作にする。chunk byte、累積 byte、deadline、in-flight native buffer を制限し、全 body の List 化や JSON array 化を避ける。結果は immutable Bytes を共有する。
3. `using` / scope cleanup / task cancel / Runtime 終了で閉じる。checkpoint の token だけで native 接続を生存させず、closed handle を fresh 操作に使うと失効エラーとする。
4. 同じ logical operation の chunk は記録または同じ in-flight read を再利用する。body を読み直すために元 request を再送しない。完了・キャンセル・閉鎖の物理 ID は巻き戻さない。
5. 実 server で多数 chunk、早期 close、途中切断、deadline、cancel、revert / resume、server 停止後 replay を検証する。ピーク memory と閉鎖後資源数を測り、Linux / Windows SDK からも source-free 実行する。

DB adapter はこの registry / async operation / typed resource の契約を再利用する。未確認の箇所を v1.6.0 の機能として扱わない。
