# v2.0 実装と受入の照合

この文書はROADMAP_v2.mdの到達条件と、実装・回帰・測定・配布検証の対応を示す。
版番号や試験件数だけを到達判定にしない。両OSの最終結果は
[Release workflow](https://github.com/viral8code/REWIND/actions/workflows/release.yml)で
確認し、成功した同一commitのtag / mainと署名付きSDKを公開する。

| 条件 | 実装と回帰 | 配布・実測での受入 |
| --- | --- | --- |
| 1. GUI・通信・DBの併用 | `src/gui`、`network.rs`、`http_server.rs`、`tcp.rs`、`database.rs`、`examples/service-{data,numeric}` | 両OSの展開後SDKでsource / debug artifact / compact artifact。両DBの独立照合、数値計算・native入力・部分失敗・切断・早期Stopを含む。 |
| 2. VMと外部作用の境界 | `runtime_external*`、`runtime_tcp`、`database_sqlite`、native leases、`begin` / publish / revert / resumeの言語回帰 | DBに確定した内容を独立照合。source / cache削除後に実行し、サービスを止めてreplay。checkpoint-onlyのtokenで物理接続を保持・復活させない。 |
| 3. 型・所有権・一般的なデータ処理 | `language_v05`以降のgeneric / trait / module / closure / borrow / effects / tasks回帰、`language_v1965` / `language_v200` | 65,536要素Listと1万超Mapのfreeze / iterator / COW / cursorのrevert。予算超過を明示し、古いlanguageのartifact挙動を維持。 |
| 4. 数値・統計・勾配 | native dense / sparse / paged storage、数値std modules、`numeric_flat`、`runtime_numeric_page_allocations`、各数値版の言語回帰 | 同期版との値・勾配一致、非ゼロ入力、checkpoint途中再開、取消後回収、モデル保存とsource-free、実GUIからの取消。 |
| 5. メモリと資源の寿命 | cached mark / sweep、共有payloadとページの会計、`runtime_gc_*`、scheduler / native resource回帰 | 64 / 256回取消、両DB各16サイクルの同時負荷。到達不能な一時値・完了task・接続を解放し、利用者が保持するcheckpointを維持。HTTP server / TCP / DB / HTTP streamの同時保持・解放も確認。 |
| 6. 費用と応答性 | native kernelの分割、paged input、freeze / List iteratorのページ共有、native rootsの重複走査削減 | 同じ最適化条件・入力でCPU / wall / peak RSS / live storageを測定。協調実行の追加費用を明記し、GUI入力と回収を同時に確認。 |
| 7. 診断・仕様・配布 | `std.taskError`、source-free診断、reference、API snapshots、effect / ownership / error / costの記述、SDK署名とhash | Linux 189依存 / Windows 191依存に対する193ライセンス見出しの照合、両OSの全回帰と展開後SDK、署名・checksum・main / tag / Releaseの一致。 |

## 候補の検証状況

- v1.9.64は両OS CI / SDKを通過し、署名付きReleaseとmain / tagの同一commitを確認済み。
- v1.9.65はローカル回帰と標準ライブラリ・展開後SDKを通過した。
  公開候補の実Microsoft日本語IMEで、preedit / convert / commitの8回反復、focus取消、
  native closeを確認した。句変換モードはサービスの既定を保持する。
  mock、Unicode確定入力、型検査で代用していない。
- v2.0のLinux候補は全892回帰（240 unit / 652 integration）、doc、標準ライブラリ、
  最適化ビルドと展開後SDKを通過した。実GUI・HTTP・両DB・数値の同時実行、
  切断・早期終了・8サイクル反復、source-free実行とサービスなしreplayを確認した。
  その後のWindows限定修正はLinuxの処理系・標準ライブラリを変更しない。
  この記録はローカル候補で確認した内容である。両OSの最終CIと
  公開Release / main / tagの一致はRelease workflowの公開判定で確認する。

## 費用の判定範囲

協調入力の262,144要素では同期版に対し約2.27倍のwall、約1.75倍のpeak RSSを
要した。これは応答性のための追加費用であり、高速化や省メモリ化と記述しない。
長時間のcompact replayにも費用がある。同時処理では必要なnative work予算を明示し、
予算を使い切った停止を成功や取消として扱わない。

4種類のnative資源（HTTP server / TCP / SQLite / HTTP download）の実接続probeでは、
所有者の存続中に4資源が保持され、解放後0、checkpoint復元後も0を確認した。
独立した逐次の最適化測定で、同じ入力を100回回収し、3回の中央値を比較した。

| rootsのInt要素数 | v1.9.64 回収時間（秒） | v1.9.65候補 回収時間（秒） | process peak RSS（KiB、両版の範囲） |
| --- | ---: | ---: | ---: |
| 16,384 | 0.06614 | 0.01647 | 10,280–10,456 |
| 65,536 | 0.31397 | 0.08090 | 17,616–17,792 |
| 262,144 | 3.52269 | 0.89663 | 46,832–47,136 |

共通roots走査の削減により、この回収区間の時間は約4分の1になった。
測定はLinuxのraw Runtime harnessで、実接続を4種類保持した場合に限る。
アプリケーション全体の速度やGC全体の速度として扱わない。process RSSはfixture開始・
入力作成・終了も含み、回収区間の使用メモリだけを示さない。各測定で終了時資源0と
checkpointによる再接続なしを照合した。協調数値入力の追加費用とは別の測定である。

v2.0のnative返却collectionの可変所有値化について、同じ非ゼロListから数値配列を
作るsource-free実行を比較した。最適化したv1.9.65 / v2.0の各SDKバイナリで、
一度warmupし、コンパイルを除いて3回逐次実行した中央値を示す。

| 要素数 | v1.9.65 wall（秒） | v2.0 wall（秒） | v1.9.65 peak RSS（KiB） | v2.0 peak RSS（KiB） |
| --- | ---: | ---: | ---: | ---: |
| 16,384 | 0.03602 | 0.03599 | 19,708 | 20,780 |
| 65,536 | 0.10119 | 0.10642 | 30,788 | 32,200 |
| 262,144 | 0.42411 | 0.46149 | 74,304 | 77,544 |

各サイズのlive数値ページは両版で同じ（312,216 / 1,240,328 / 4,951,672 bytes）だった。
この入力変換では最大約9%の時間、約4〜5%のpeak RSS増加がある。通常の所有Listとして
変更できる機能には費用があり、追加heap headerや保持・借用の処理を無料とは扱わない。
process測定には起動・入力準備・profile出力も含む。全処理の速度やGC時間の比較へ広げない。
再現用の処理は`scripts/benchmark-numeric-input.py`を使用した。

全てのGUI・数値・DB仕様への無限定な保証は行わない。AT-SPI / MSAAの提供範囲、
深さ・サイズ・予算、debug記録の上限、外部commitの不明状態、手動retryの契約を維持する。
