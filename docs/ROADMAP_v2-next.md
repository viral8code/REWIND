# REWIND v2.1以降の開発計画

この文書は未実装の計画です。基準は公開済みv2.0.0、実装commit `d0f3ee6684769d3f92dfb0e21c55deffc00dc181`です。入門書追加commit `5f5ab8f`は文書の整備であり、新しい言語版ではありません。

以前の[2.0までの計画](ROADMAP_v2.md)は過去の計画として残します。現在の仕様は[入門とリファレンス](book/index.html)、提供範囲と検証の履歴は[v2.0仕様](REWIND_v2.0.md)と[受入照合](v2-acceptance.md)を参照してください。過去の「未実装」を新しい不足一覧へ転記しません。

## 目的と優先順位

既存機能を組み合わせて開発するときの調査・操作・保守の負担を減らします。新しい構文やmodule名を増やすこと自体を達成条件にはしません。

| 優先 | 対象版 | 主題 | 完了時にできること |
|---|---|---|---|
| 1 | 2.1 | 開発支援とCLIの一貫性 | エディタで記録を前後に調べ、task・値・診断を追い、同じroot指定で同じ現行仕様を使える |
| 2 | 2.2 | GUIとDBのアプリケーションAPI | 再利用する画面を組み立て、型付き行変換と明示transactionで入力・保存・失敗を扱える |
| 3 | 2.3候補 | 性能・保持領域・長時間運転 | 実測で問題の処理を特定し、外部境界と数値精度を保ってコピー・走査・待機費用を減らせる |
| 保留 | 後続版 | 表・数値処理の追加ライブラリ | 必要性を実例で確認した操作を、既存numeric/stream等を再利用して提供する |

版とAPI名は設計段階の候補です。検証可能な作業単位に分けますが、各作業を機械的にpatch releaseへしません。互換性の不具合修正だけを先行公開する必要がある場合はpatch版を使います。

## 既存基盤を再実装しない

| 分野 | 2.0.0で利用できるもの | 次に改善する差分 |
|---|---|---|
| デバッグ | recorded debugger、breakpoint、前後step、task選択、checkpoint/state/files/diff、source-free記録 | 記録閲覧engineの再利用、DAP連携、値の安全なページ表示 |
| 診断 | source位置、caller、hints、causes、typed budget、wait graph、JSON、TaskError envelope | 所有権・効果・継続の理由説明、エディタとCLIの情報一致 |
| CLI/LSP | 簡易run/compile、project、fmt、doc、LSP、署名付きSDK | manifestなし明示rootの現行仕様、help/診断/移行手順の統一 |
| 性能・メモリ | logical profile、GC、永続List/Map/heap、COW、native配列、共有領域会計 | 費用の帰属、checkpointによる保持理由、長時間の測定と改善 |
| GUI | native window、grid配置、複数画面、form検証、表viewport、clipboard、menu/dialog、IME、協調入力、accessibility | 構成可能なlayout、再利用component、編集可能な表、表示・検証の統合 |
| DB | SQLite/PostgreSQL、binding、prepared statement、cursor/batch、typed cell変換、transaction、cleanup | recordへの明示mapping、transaction helper、migrationの限定API |
| 数値 | dense/sparse配列、linear algebra、統計、FFT、自動微分、optimizer、model保存、協調kernel | 既存APIを使う可視化、計測で確認した変換・一時領域の削減 |

2.0.0のnative GUIやDB、async kernelが「ない」という前提で計画しません。高機能化する部分と、既存実装を確認する部分を分けます。

## 全版を通して維持する契約

1. VMの変数・heap・model・pending I/Oと、観測記録・公開済み作用・physical資源の寿命を区別する。
2. デバッガーの表示やwatchで通信、SQL、publish、callbackを勝手に実行しない。
3. DB commit/rollbackをVM commit/revertへ置き換えない。結果不明の書込みを自動retryしない。
4. Secretの既存redactionを、デバッガー・profile・新しいerror表示でも維持する。
5. checkpointが保持する到達可能値をGCで捨てない。共有ページを重複計上する説明と実装にしない。
6. heavy処理は既存native配列と協調APIを再利用する。純粋なREWIND callbackを外部workerへ無条件に移さない。
7. APIには効果・所有権・容量・単位・失敗時の状態・費用を記載する。
8. language互換とartifact/trace形式の互換を分ける。旧形式を黙って新形式へ読み替えない。

外部process実行、VM外の任意の巻き戻し、JVM互換、project管理の大型拡張は本計画の優先項目に含めません。GPUや新しいDB backendなどは、先に既存利用の不足を測定してから別案で判断します。

## 詳細計画と再開先

- [v2.1：CLI・診断・デバッガー・profile](REWIND_v2.1-plan.md)
- [v2.2：GUI・可視化・DB helper](REWIND_v2.2-plan.md)
- [性能改善と測定計画](v2-next-performance.md)
- [作業順序・状態・次回再開手順](v2-next-status.md)
- [追加構想：言語表現・型・変更履歴](v2-next-language.md)
- [追加構想：通信・サービス・外部資源](v2-next-services.md)
- [追加構想：表データ・数値処理・一般ライブラリ](v2-next-data.md)
- [追加構想：配布・開発品質・標準ライブラリの継続運用](v2-next-ecosystem.md)
- [追加34候補の優先順位・依存・採用判断](v2-next-selection.md)

- [追加構想：partial・生成・型の抽象化](v2-next-type-architecture.md)

## 構想を採用する順序

追加構想は34候補です。既定のv2.1/2.2と性能段階を優先し、型付き検証・schema・stream接続・互換性照合など、既存機能をつなぐ候補から調査します。名前付き引数、列指向table、双方向通信、追加solver、native拡張等は最小例と費用を確認して採否を決めます。全候補を公開条件にせず、版番号も先に割り当てません。

詳細な優先順位と採用templateは[選定表](v2-next-selection.md)にあります。

## 文書・実装・公開の扱い

今回の依頼は計画の文書化です。runtimeの変更、版番号の更新、SDK再公開は行いません。作業branchは`codex/develop`だけです。文書をまとまりごとにcommit/pushし、commitには`[skip ci]`を付けます。mainへは今回統合しません。

実装を始めるときも、最初は対象を絞ったローカル検証で結果を確認します。現在の「公開CI/CDを動かさない」という指示がある間は、workflow dispatch、release用tag、release開始message、公開workflowを起動する操作をしません。正式公開の検証手順を計画に記載することは、今それを実行する許可ではありません。
