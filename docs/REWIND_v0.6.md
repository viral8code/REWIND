# REWIND プログラミング言語設計案 v0.6

作成日: 2026-09-30。

状態: **提案・未実装**。v0.5 の実装規則は [v0.5-status.md](v0.5-status.md) を参照する。以下の構文は確定していない。

## 1. 目的と優先順位

v0.5 で所有権の検査、Frozen、関数効果、検証付き成果物、関連型、Iterator、編集・記録支援を導入した。v0.6 は、ライブラリを組み合わせたときの保守的な制限を減らし、日常的な開発と配布に必要な機能を整える。

| 優先 | 要素 | 改善する場面 |
|---|---|---|
| 必須 | 効果を持つ関数型と効果パラメータ | 高階関数に全能力を要求せず、必要な効果だけを伝播する |
| 必須 | 借用引数と戻り値の所有権契約 | コレクションを move/freeze せずに読み取り・更新する |
| 必須 | 型付き診断とエラー連鎖 | cancellation、cleanup、複数タスクの失敗を失わず扱う |
| 推奨 | Iterator adapter、標準ライブラリの拡張 | map/filter/fold、文字列・数値変換を通常コードで記述する |
| 推奨 | 永続的な増分ビルドと配布形式の互換方針 | 大きなプロジェクトと compiler 更新を扱う |
| 推奨 | LSP と debugger の拡充 | 修正、参照、再生による原因調査を編集環境で完結する |

## 2. 関数型の効果

暫定例:

```rewind
fn apply<E>(f: fn(Int)->Int effects E, n:Int) -> Int effects E {
    return f(n);
}
```

- 関数値・クロージャーの型に効果集合を持たせる。
- 効果変数を型変数と区別し、包含制約と推論規則を定義する。
- trait のメソッド契約に効果の上限を持たせ、impl が上限を超える場合は拒否する。
- generic の単相化後には実際の実装の効果を使用し、無関係な impl の効果を混ぜない。
- 別名、再代入、match、戻り値、コンテナに入れた関数値でも効果を保持する。
- publish は効果変数で library/async に持ち込めない境界操作として維持する。

完了条件: pure closure を apply して能力なしで実行できる。I/O closure の効果不足には、呼び出し元から操作までの経路を示す。再代入や trait を経由して権限を隠せない。推論の循環は明示予算で停止する。

## 3. 所有権契約と借用

暫定例:

```rewind
fn total(xs: &List<Int>) -> Int effects {} { /* ... */ }
fn append(xs: &mut List<Int>, n:Int) -> Unit effects {} { /* ... */ }
```

- 借用引数をシグネチャに表し、呼び出しの間だけ保持する。
- 最初は借用を返す関数を禁止し、所有する値を返す契約を明確にする。後で必要性を検証して寿命パラメータを検討する。
- 部分移動、フィールドの借用、再借用、コンテナへの挿入・取り出しの規則を統一する。
- クロージャーの捕捉を value/move/borrow で明示できるようにし、Send/Share をシグネチャに保存する。
- Checkpoint の世代をまたいだ借用、resume による寿命の再開、Channel 転送の境界を検査する。
- 非同期 API の借用は最初は拒否し、同期の借用引数と移動によるタスク所有を先に安定させる。

完了条件: read-only API が freeze のコピーを必要としない。可変借用の排他性を分岐・反復・cleanup まで検証する。元所有者の更新・移動・破棄と競合する参照を実行前に拒否する。

## 4. 診断とタスク失敗の集約

- Diagnostic を文字列から、code、source span、元エラー、cleanup 原因、task ID を持つ木へ進める。
- WaitGraph を task-to-task/channel/group の型付き辺として公開する。
- BudgetKind は scheduler storage、transfer depth、型展開、単相化、history memory/storage を区別する。
- TaskGroup の複数の失敗を順序付きで返し、第一原因を保ちながら残りの原因を確認できるようにする。
- cancel request、cancel-and-join、timeout に相当する論理的な選択を分離する。実時間時計に依存するタイマーは追加しない。
- Result の明示的な map/mapErr/andThen と、二層 Result の原因を保持した flatten を提供する。

完了条件: 親の `?`、panic、自己取消し、予算超過、複数 cleanup 失敗を含む実行と replay が同じ原因木になる。失敗の表示に秘密値を含めない。

## 5. 標準ライブラリと型表現

言語として次に必要になる候補:

1. **Iterator adapter**: map/filter/enumerate/zip/take/fold/collect。独自 Iterator 型を associated type で組み合わせる。
2. **文字列と Bytes**: 長さの単位を区別し、UTF-8 decode/encode、分割、検索、slice を失敗型付きで提供する。
3. **数値変換**: parse/format、checked conversion、NaN・無限大・overflow の規則を定義する。
4. **データ表現**: tuple、型 alias、必要なら named arguments。Option/Result との整合性を先に決める。
5. **trait の default method**: 明示 override と重複の選択規則を固定する。動的 trait object は別途検証する。
6. **標準テスト API**: assert の差分表示、入力生成、失敗例の縮小、再現 seed を持つ property test。

すべての API に型、効果、失敗、checkpoint 復元時の意味、時間・保存領域の上限を記載する。

## 6. ビルド、配布、パッケージ

v0.5 のメモリ内モジュール解析キャッシュと成果物キャッシュを、コマンドをまたぐ増分ビルドへ拡張する。

- module content、公開 API、効果、依存、compiler をキーにする。
- 変更の影響を受けるモジュールだけを再解析・再検査・再コンパイルする。
- cache は信頼境界として扱い、署名付きソースの検証を任意の cache IR で迂回できない設計にする。
- 全再構築と増分ビルドで byte が一致することを検証する。
- compiler と artifact/lock/trace の版を分離し、読める版、変換可能な版、拒否する版を表で示す。
- 複数のローカル候補から、制約と推移依存に合う決定的な版を選択する。更新案の preview と適用を分離する。
- 明示 fetch を VM 外のツールとして設計する場合は、固定 URL/内容ハッシュ、mirror、offline、取得上限を必須にする。
- 鍵の rotation/revocation の根拠を持つ trust metadata を検討する。署名が有効でも効果・型・版検査は維持する。

完了条件: dependency graph が変化した場合の再検査漏れがない。破損 cache は再構築され、破損 artifact は拒否される。更新候補の理由と trust の変更が利用者に分かる。

## 7. 編集・デバッグ・秘密

- LSP: local/module の定義と参照、rename、signature help、保存前の差分同期、診断の関連箇所、move/effects の修正候補。
- debugger: 命令・ソース行単位の移動、逆方向 step、breakpoint、タスク選択、待機グラフ、仮想ファイルの内容差分。
- source map と検証済み artifact を trace に結び、外部ソースがない配布物でも命令ごとの状態を再構成する。
- prefix の毎回再生に代えて、保存予算内の checkpoint index を使う。表示で元の Journal、record、Host を変化させない。
- Secret の派生をメソッド、フォーマット、診断、コンテナまで一貫して追跡する。reveal の責任と監査記録を明示する。
- 秘密観測の外部供給を拡充する。暗号化保存を選ぶ場合は鍵管理、認証付き暗号、nonce、再生時の必要情報を別仕様にする。

完了条件: 日本語と supplementary Unicode の UTF-16 位置が一致する。秘密を含む待機状態・原因・ファイル差分を伏せる。前後移動と breakpoint によって実行の観測が増えない。

## 8. 工程、採否、対象外

実装順: 効果付き関数型 → 同期借用引数 → 型付き原因木 → Iterator/変換 API → 増分ビルド → LSP/debugger。各段階で v0.1〜v0.5 の回帰と、同一入力の決定性を確認する。

採用前に決めること:

- 効果変数を宣言する構文と推論の上限。
- borrowed return を導入する必要性と Checkpoint の世代との関係。
- TaskGroup の失敗集約型と取消しの優先順位。
- artifact/trace の互換期間、cache の検証方法。
- Secret の解除・外部供給・暗号化の役割分担。

VM のネットワーク、DB、FFI、OS 並列スレッド、子プロセス、GPU、デバイス、実時間タイマー、JIT はこの提案の必須要件に含めない。

この文書は v0.6 の実装済み機能を示さない。
