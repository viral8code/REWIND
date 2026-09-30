# REWIND プログラミング言語設計案 v0.7

作成日: 2026-09-30。状態: **採用範囲を実装済み**。実装した構文・コマンド・上限は [v0.7-status.md](v0.7-status.md) を参照。

v0.6 の実装範囲と制限は [v0.6-status.md](v0.6-status.md) に記載する。本書はその実装を前提に、次の設計候補を整理する。採用する構文・互換性・実装順は v0.6 の検証結果を踏まえて確定する。

v0.6 では tuple pattern・type alias・default method、公開 Send/Share 契約と一時借用捕捉、論理 timeout/select、整数 property test、増分 cache、更新 preview/apply、LSP 編集支援、source-free 記録 index と逆方向デバッグ、秘密入力と監査を実装した。本書の pattern・寿命・公開契約の提案は、この実装を前提に検証する。

## 1. モジュール境界で使える言語

### 不変データと pattern

- immutable record、enum の payload、v0.6 の tuple destructuring を統一する。
- pattern による部分移動と借用の規則を定義し、分岐の網羅性・到達不能な arm を診断する。
- const evaluation は純粋な式に限定し、実行手順・型展開・保存領域の予算を明示する。

完了条件: const の評価結果が通常評価と一致する。pattern の一部が移動された後に、元の値全体を使用できない。

### 寿命・地域・効果の公開契約

- borrowed return の必要性を実例で検証し、導入する場合は寿命パラメータを公開 API に保存する。
- Checkpoint 世代をまたぐ参照は地域の契約で制限する。resume による参照の再利用は世代一致を要求する。
- 関数の effect / ownership / Send / Share 契約の変更を API 差分として表示する。

完了条件: library の契約違反を呼び出し元で検出でき、型表示だけで必要な能力と所有権が分かる。

## 2. 失敗と復元を扱う API

- recoverable failure と panic を区別し、Result と原因木を保持した変換を設計する。
- task scope の終了規則、取消し要求と終了の違い、複数失敗の順序を公開契約にする。
- REPL で仮想 I/O と Checkpoint を扱い、失敗した入力だけを取り消せる実行モデルを検討する。

完了条件: 記録・再生・対話実行が同じ失敗木と観測列を生成する。失敗の取り消しで Host の変更を発生させない。

## 3. ライブラリ作者のための仕組み

- module の公開 API snapshot と互換性検査。型だけでなく効果・所有権・失敗型・既定動作を比較する。
- trait の coherence と specialization の範囲を固定する。動的 trait object は所有権・効果契約を保存できる場合に検討する。
- package の用途別依存、開発用依存、公開 API 文書のリンクと example の実行を設計する。
- property test の生成器を型ごとに組み合わせ、再現 seed と縮小済み入力を replay 記録に保存する。

完了条件: library 作者が依存を更新する前に破壊的変更を確認できる。文書の example が独立した仮想環境で実行できる。

## 4. 配布と開発環境

- 独立した artifact/lock/trace の互換期間と変換ツールを定める。
- v0.6 の protocol test を拡充し、未完成ソースの意味解析・import alias の rename・editor integration example を整える。
- v0.6 の記録 index に基づく debugger に、複数 task の原因木と仮想ファイル変更をまとめた時間軸 UI を追加する。
- opt-in の暗号化 trace を導入する場合は、鍵の供給・認証付き暗号・nonce・復元に必要な情報を別仕様にする。

完了条件: ソースなしの成果物について、実行・再生・デバッグの互換可否をコマンドで説明できる。暗号鍵と秘密値を repository や通常ログに保存しない。

## 5. 採否と順序

1. v0.6 の完了条件と未解決の制限を検証する。
2. API 契約の差分と文書 example の実行を先に実装する。
3. pattern/const/対話実行を独立した仕様として追加する。
4. borrowed return、動的 trait object、暗号化 trace は実例と失敗条件を確認してから採用する。

VM 内のネットワーク、FFI、OS 並列スレッド、子プロセス、実時間 timer、JIT は引き続き本提案の必須要件に含めない。

## 6. 採用結果

API snapshot/diff、独立した仮想 I/O の doctest、不変 record、純粋な const 評価検査、合成 property generator、source-free JSON timeline を採用した。部分移動・借用と原因木は v0.6 の規則を継承する。用途別依存・対話実行・読み取り専用の互換期間と export は [v0.8 案](REWIND_v0.8.md) に分離する。borrowed return・動的 trait object・暗号化 trace は条件付き候補であり、この版の採用機能には含めない。
