# 追加構想の選定表

状態：採用判断前。2026-10-10時点の候補一覧です。[全体計画](ROADMAP_v2-next.md)のv2.1/2.2詳細と性能計画が優先です。候補をすべて実装する約束や、特定版の公開条件にはしません。

## まず取り組むまとまり

1. **開発中に原因を追える状態**：既定のCLI-01、診断、記録DAP、profileを進める。追加候補はDOC-01とDIST-03の照合基盤。
2. **入力・一覧・保存を少ない反復記述で作る**：既定のGUI/DB helperと、LANG-06・DATA-01の検証/mappingを整合させる。
3. **失敗と終了まで扱える小さなサービス**：NET-01/02、SVC-01/02、QUAL-02を選ぶ。外部結果不明を隠す自動retryは作らない。
4. **大きな入力を全件コピーせず変換する**：性能baseline、DATA-04、NUM-01を先行し、必要が確認できたらtable/groupを選ぶ。
5. **作ったものを配れる状態**：DIST-01/02を確認する。native拡張や新backendは必要性と配布費用を見て別に判断する。

これらは選定のまとまりです。各まとまりに構文・runtime・library・文書が混在するため、実装commitは最小のreview可能な差分へ分けます。

## 評価の読み方

- **A**：既定計画との接点が多い。最小例による調査を早めに行う。
- **B**：具体的な不足を確認してから選ぶ。
- **C**：独立した設計・費用・運用責任が大きい。先に採用しない。
- 規模S/M/Lは相対的な設計範囲です。工数見積りや完了日ではありません。
- 同じAでも依存関係を守ります。未確認の既存機能は不足と断定しません。

## 言語・状態：9候補

詳細は[言語構想](v2-next-language.md)。

| ID | 候補 | 優先 / 規模 | 採用前に示す根拠 |
|---|---|---|---|
| LANG-01 | 関数の名前付き/default引数 | B / M | 設定recordでは解消しない誤用例と評価順 |
| LANG-02 | error型の明示変換と伝播 | B / M | mapErrorの反復例、一意性、causeの保持 |
| LANG-03 | 型付きID/単位のwrapper | A / S | 既存private型での成立例と誤用拒否 |
| LANG-04 | generic/fallible collection helper | A / M | 現行helperで扱えない型・callbackと費用 |
| LANG-05 | ownerを消費するResult/Option操作 | B / M | affine値の安全なsignatureとcleanup |
| LANG-06 | 複数errorを集める検証 | A / M | form/設定/schemaで共用するbounded結果 |
| STATE-01 | 明示容量付きUndo履歴helper | B / M | label/継続/branch契約での成立例 |
| STATE-02 | publish結果の型付きadapter | A / M | 取得可能な送信状態と部分適用の利用例 |
| LANG-07 | deprecationと移行通知 | B / M | 診断schemaとSDK差分の一貫性 |

LANG-03/06はlibrary/例で成立すればcompilerを変更しません。LANG-01/02は便利さとsource互換の両方を判断します。

## 通信・サービス：8候補

詳細は[サービス構想](v2-next-services.md)。

| ID | 候補 | 優先 / 規模 | 採用前に示す根拠 |
|---|---|---|---|
| NET-01 | URL/query/path helper | A / M | 現行解釈との一致と二重decode防止 |
| NET-02 | header/cookie/body helper | B / M | bounded parserとstream境界の契約 |
| NET-03 | 明示retry policy | B / M | 再送可能条件、deadline、unknownの扱い |
| SVC-01 | compiled routerと入力schema | A / M | exact routerとの差、曖昧routeの拒否 |
| SVC-02 | request scopeとshutdown | A / L | 子task取消、resource回収、physical期限 |
| SVC-03 | structured log/統計 | B / M | 公開境界、redaction、bounded queue/label |
| SVC-04 | credential切替とpool | C / L | 世代/貸出/cleanup/上限会計の成立 |
| NET-04 | 双方向protocol | C / L | 実用例、frame/queue/cancelの有限上限 |

NET-02の圧縮、SVC-04のpool、NET-04はそれぞれ独立した大きな追加範囲です。一つの便利関数として契約を隠しません。

## データ・数値・一般library：9候補

詳細は[データ構想](v2-next-data.md)。

| ID | 候補 | 優先 / 規模 | 採用前に示す根拠 |
|---|---|---|---|
| DATA-01 | 型付きschema/mapping | A / M | JSON/CSV/DBの共用例と精度/NULL |
| DATA-02 | 列指向table | B / L | per-cell費用、native列とsnapshot所有権 |
| DATA-03 | group/aggregate/join | B / L | DATA-02、順序/NULL/出力増大の上限 |
| DATA-04 | bounded stream pipeline | A / M | parser/cursorの接続例とbackpressure |
| NUM-01 | kernel間の変換削減 | A / M | baseline、コピー箇所、誤差/取消契約 |
| NUM-02 | 不足solver | C / L | 現行solverで解けない例と参照解 |
| NUM-03 | 学習/評価helper | B / M | 現行trainingとの組合せ、再現性、保持 |
| LIB-01 | algorithm APIの監査とadapter | B / M | 既存libraryの具体的な制約と参照解 |
| LIB-02 | codec/圧縮/画像 | C / L | 形式の需要、展開上限、license/配布 |

DATA-04とNUM-01を先に調べ、大きなtable frameworkやsolver追加が本当に必要かを判断します。

## 配布・開発品質：8候補

詳細は[配布・品質構想](v2-next-ecosystem.md)。

| ID | 候補 | 優先 / 規模 | 採用前に示す根拠 |
|---|---|---|---|
| DIST-01 | 最小runtime/offline bundle | B / M | artifactの実依存とclean環境動作 |
| DIST-02 | Windows配布matrix | A / M | 実Windowsでのpath/GUI/TLS/DB確認 |
| DIST-03 | API/形式互換性matrix | A / M | 互換性の軸と既知fixtureの判定 |
| QUAL-01 | property test/縮小 | B / M | seed再現、縮小予算、参照実装 |
| QUAL-02 | application向け失敗fixture | A / M | 実backendとの契約一致、fallbackなし |
| QUAL-03 | coverage/失敗調査 | B / M | source対応、無効時費用、bounded記録 |
| EXT-01 | native adapter契約 | C / L | 所有権/会計/ABI/取消/cleanup |
| DOC-01 | referenceの継続照合 | A / S | 現行book生成/例検証の再利用 |

macOS/ARM、localization、accessibility拡張、library catalogは補助候補です。新OSや巨大project管理をここで必須条件に追加しません。

## 採用・保留の記録

次のtemplateで候補ごとに判断を残します。

~~~text
候補ID:
状態: 採用判断前 / 調査中 / 採用 / 保留 / 既存機能で解決
利用者の困る操作と最小例:
現行APIで可能な部分:
本当に追加する差分:
依存する作業ID:
型・effect・所有権・revert/physical境界:
費用と容量/精度/取消条件:
互換性・配布への影響:
受入例と確認環境:
採用理由、または保留理由と解除条件:
実装対象版: 未割当なら未割当
~~~

採用した項目は詳細設計と受入を確定してから[v2-next-status](v2-next-status.md)の実装一覧へ移します。「採用判断前」と「実装の未着手」を混同しません。保留や既存機能での解決も正当な結果です。

## 版番号の扱い

v2.1/2.2は既存詳細計画、性能段階はv2.3候補のままです。この34候補に将来のminor番号を先に割り当てません。採用したまとまりの完成条件・source/API/形式互換を見て決めます。

小さな作業ごとのcommitと、利用者へ公開するversionは別です。完了していない大きな課題を隠すためにpatch番号を上げません。今回の変更は計画だけで、公開CI/CD、Release、main統合は行いません。
