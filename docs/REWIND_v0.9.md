# REWIND v0.9 作成案

作成日: 2026-09-30。状態: **採用範囲を実装済み**。採用したコマンド・構文・上限は [v0.9-status.md](v0.9-status.md) を参照。v0.8 の完了範囲と上限は [v0.8-status.md](v0.8-status.md) を参照。本書の条件付き候補と採用機能を末尾で区別する。

## 1. 対話実行と観測の契約

v0.8 の prefix 再実行を基準に、入力ごとの変更と typed failure を持つ session transcript を定義する。accepted source、失敗入力、観測の再利用、Checkpoint の世代を区別する。source の指紋を固定する通常 replay の規則は弱めない。

複数行入力の終了条件と入力サイズの上限、未完ソースの補完・hover・signature help を整える。VM 継続を保存して prefix の計算を省く方式は、クロージャー・単相化・移動状態・Checkpoint・cleanup を同時に保存できる場合に採用する。live task と定義置換は別仕様にし、最初の transcript には含めない。

完了条件: 同じ transcript から accepted state と失敗の原因木を再構成できる。未完の入力で前の状態と Host を変更しない。実行予算は入力と session の両方で説明できる。

## 2. ライブラリ公開契約と配布

API snapshot に public impl・関連型の実装・公開 API が参照する依存契約を含める。const は initializer の構文差分と評価値の変更を分け、trait default の同等性は保守的な判定を維持する。snapshot format の世代変更には専用の変換器と拒否条件を設ける。

production-only install を設計し、通常 graph だけの package 配置でも署名済み lock の runtime 部分を検証できるようにする。開発 graph の欠落と tampering を混同せず、test/doctest の開始時は開発 graph の完全検証を要求する。

完了条件: 依存由来の公開契約の破壊を update 前に検出できる。通常実行から開発依存の module を path で迂回できない。未検証の開発 lock を実行 artifact に混入しない。

## 3. データと所有権の拡張候補

- generic record の型パラメータに Share 契約を要求し、フィールドの深い不変性を定義する。
- property generator の再利用可能な型と、複数候補を返す shrinker を設計する。探索順・重複除去・予算を記録し、非単調 predicate の大域最小は保証しない。
- borrowed return は実例を集め、寿命と Checkpoint 地域の公開契約を定義できる場合だけ採用する。参照の再利用には世代の一致を要求する。
- 動的 trait object は receiver の所有権、Send/Share、関連型、効果上限を artifact に保存できる場合だけ採用する。既存 coherence を変更する場合は API breaking とする。

完了条件: generic record と property 入力の transfer 契約を呼び出し側・artifact 検証の両方で確認できる。返却参照と task 転送で所有者の寿命を越えられない。

## 4. 記録表示と秘密

0.9 の読み取り専用期間は indexed trace 0.8/0.9 を基本とし、0.7 の長期保存は inspection bundle を使う。inspection bundle の独立した署名形式、署名検証済みフラグと改変検出の区別を仕様化する。実行/replay の exact compiler 制約は継続する。

timeline の署名検証 option、安定した JSON schema、task 因果関係・原因木・仮想ファイル変更の editor 表示を加える。元記録の観測 journal を公開する際は、秘密の再供給やファイル内容の扱いを明示する。

opt-in 暗号化 trace は条件付き候補。採用前に認証付き暗号、nonce 一意性、鍵供給、associated data、key rotation、破損・鍵欠落時の失敗、復号予算を別仕様にする。独自暗号は作らず、暗号鍵を repository/通常ログに保存しない。暗号化は公開済みの出力を取り消す仕組みにはならない。

## 5. 実装順の提案

まず transcript と未完ソースの編集支援、公開契約の依存追跡、production graph 検証を実装する。次に generic record と property shrinker を追加する。borrowed return・動的 trait object・暗号化は実例と失敗条件をレビューして採否を確定する。

VM 内のネットワーク、FFI、OS 並列 thread、実時間 timer、子プロセス、JIT は必須要件に含めない。

## 6. 採用結果（2026-10-01）

transcript の記録・観測限定 replay、複数行 REPL と session 予算、未完ソースへの最後の検査済み宣言による補完・hover・signature help、API snapshot format 2 と検査済みソースを使う変換、production-only 配布、Share generic record、複数候補の property shrinker、inspection/session の独立署名、署名付き timeline と editor 用 JSON request を採用した。

REPL は prefix 再実行を継続し、live task・定義置換・VM 継続の保存は採用していない。未完ソースの支援は最後に検査できた宣言を明示し、新しい未検査の宣言を意味解析済みとして扱わない。borrowed return・動的 trait object・暗号化 trace は条件付き候補のままである。アプリケーション開発に向けた不足と次の優先順位を [v0.9.1 草案](REWIND_v0.9.1.md) にまとめた。
