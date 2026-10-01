# 実装ロードマップ（v1.4〜v2.0）

各版は実装、回帰テスト、標準ライブラリ契約、Linux / Windows SDK の検証、Release 公開、main への統合まで進める。未実装機能を実装済みとして記載しない。

## 状態と外部作用の境界

VM の値、heap、cursor、未公開 I/O は checkpoint の対象。観測済み入力と公開済み結果は外部の事実として保持する。ネットワークや DB に対する処理を VM の巻き戻しへ偽装しない。外部操作は明示した領域・effect で実行し、成功・失敗・部分適用、入力の記録と replay、接続資源の寿命を区別する。外部の操作を再試行・再送しないことを既定とし、再実行が必要なときは利用者が明示する。

## 段階

| 版 | 実装範囲 | 主な検証 |
| --- | --- | --- |
| 1.4 | GUI 文字編集、選択、Unicode 入力、配置・resize、入力 polling、scene コピー削減、begin 境界の診断 | native 入力、編集 / Undo / 保存、fixture と replay |
| 1.5 | 外部作用の明示領域、非巻き戻し操作の ledger、記録 / replay と失敗の契約 | 二重送信防止、部分失敗、境界の静的検査 |
| 1.6 | HTTP / HTTPS、制限付き body、timeout、TLS 検証、header と status の型付け | ローカル HTTP server、TLS エラー、再送しない replay |
| 1.7 | SQLite 接続、parameter binding、型付き行、明示 transaction、資源の確実な解放 | DB transaction と VM checkpoint の独立、rollback、replay |
| 1.8 | Float 数学、dense 数値 buffer、線形代数、統計、データ変換 | 数値誤差、境界、計算量、代表的な数値処理 |
| 1.9 | 自動 GC / checkpoint root の監査、実行時の高速化、協調タスク、データ・型の実用性 | 回収・循環・保持、連続動作、性能 / memory benchmark |
| 2.0 | 統合、仕様・診断・標準ライブラリの不足修正、数値処理の拡張、SDK の安定化 | GUI + DB + HTTP の統合、各ライブラリ、source-free 配布、両 OS |

## 到達確認

言語の制御構文、型、generic、closure、所有権、エラー処理、タスク、標準 collection を現状と比較して監査する。既にある機能の再実装は行わない。新機能と既存機能の組み合わせを実プログラムで確認する。

数値処理は実データに対して線形代数、統計、最適化、勾配・損失関数・反復計算を実行する。型付き数値 buffer の内側の loop は必要に応じて Rust へ置き、VM instruction や heap object の増幅を避ける。機械学習用途の kernel を、汎用的な巨大依存や別処理系の起動へ置き換えない。

GC は current roots、各 checkpoint、branch、task、closure、native resource を区別して測定する。checkpoint が保持する到達可能データは利用者の履歴として維持し、到達不能データと一時的な native buffer は解放する。処理速度、最大 live heap、RSS と保持する履歴量を benchmark に記録する。公開時の保証範囲は測定・検証された範囲とする。
