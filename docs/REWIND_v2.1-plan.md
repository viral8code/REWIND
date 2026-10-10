# REWIND v2.1 計画：開発支援とCLIの一貫性

状態：未実装。基準と優先順位は[次版ロードマップ](ROADMAP_v2-next.md)を参照。対象は2.0.0にある機能の拡張であり、デバッガー・診断・profileの新規作り直しではありません。

## 到達条件

1. 現行languageの単一sourceを、root省略・明示の両方で型・効果・実行結果が一致する形で扱える。
2. 所有権、効果、checkpoint継続、子taskの失敗を、CLIとエディタで同じcode・位置・原因として調べられる。
3. エディタからdebug記録を前後に移動し、breakpoint、stack、task、checkpoint、値を安全に閲覧できる。
4. profileで仕事量と保持領域の帰属を確認でき、観測そのものが外部作用やruntime状態を変えない。
5. Windows/Linux、source/artifact、記録/replayで契約を確認し、現行の入門書とAPIを更新できる。

初版のエディタ連携は**記録を閲覧するデバッガー**を対象にします。実行中processへのattachや、任意命令位置でのlive停止までを達成済みとは扱いません。

## 既存実装と主な変更対象

| 対象 | 現在の基盤 | 変更候補 |
|---|---|---|
| CLI | `src/main.rs`のroot選択、`RunOptions.standalone` | sharedな実行context解決とhelp |
| checker | `src/v2.rs`、`src/v2/v05/ownership.rs`、`src/v2/v05/capabilities.rs` | 関連位置・理由・提案を持つ診断 |
| runtime診断 | `src/v2/v06/diagnostics.rs`、`std.taskError` | 原因を失わない表示・LSP/DAPへの変換 |
| recorded debugger | `src/v2/v05/tooling.rs`のdebug session | command処理と記録viewをtransportから分離 |
| editor | `src/v2/v05/tooling/`と既存LSP | 共通位置変換、DAP server、最小client設定例 |
| storage/profile | `src/lib.rs`、`src/shared_payload.rs`、既存profile | counter帰属、共有領域とrootの説明 |

実装開始時に関数・fileの現在位置を再確認します。行番号を固定した作業指示にはしません。

## A. CLIのrootとlanguage解決

### CLI-01：context解決を一本化する

2.0.0の単一sourceは、root省略時に親からmanifestを探索し、なければ現行standaloneになります。一方、manifestなしの明示rootは旧project経路へ入り、現行構文やstdの解決が変わります。これを2.1のCLIで解消します。

- `run`、`compile`、`check`、`doc`、sourceを扱うdebug/profileが同じcontext resolverを利用する。
- manifestがある場合はlanguage、source_root、entry、effects、lockを優先する。不正manifestを「なかった」とみなさない。
- manifestがない明示rootではrootをFile/importの境界として使い、現行standaloneを選ぶ。
- sourceがroot外の場合、相対import・File権限と同様に意図を検査し、黙ってrootを広げない。
- artifact実行/replayは保存されたlanguage・fingerprintを尊重し、CLIの現行defaultへ読み替えない。
- cwd、sourceの親、明示rootの区別をhelpと診断へ残す。

**互換性判断：** manifestなし明示rootに依存した旧modeは挙動が変わるため、旧languageを明示したmanifestを用意する移行例を出す。署名lockや公開形式の互換保証は別に扱う。現在の不具合をpatchとして直す場合は、この変更範囲と旧利用の扱いを先に決める。

**検証：** 親manifestあり/なし、壊れたmanifest、異なるcwd、root内外、relative std/user import、Unicode/空白を含むpath、Windows driveのsource指定、ソース削除後artifact、replay。型検査だけと実行でlanguageが変わらないこと。

### CLI-02：helpと機械可読エラー

- doc等、現在subcommand helpが一律でない入口を整理する。
- `--`以降はapplication引数という規則を維持する。
- 不明option、値不足、不正数値、非対応OS optionを、利用説明と失敗codeが分かる形に揃える。
- JSON診断はstderr、application outputはstdout。helpはsourceを読まずに終了する。
- runtime/source/compiler/OSの予算の違いをhelpへ簡潔に示す。

**検証：** helpでFile/通信が発生しない、`-- --help`はapplicationへ届く、subcommandの組合せと終了値、text/JSONのcode一致。

## B. 診断を開発中の判断へ結び付ける

### DIAG-01：所有権・効果の関連位置

移動後利用には「移動した式」と「再利用した式」、借用競合には「借用開始」と「競合変更」、効果不足には「要求した呼出し」と「足りない宣言/実行許可」を示す。primary codeは維持し、関連位置を追加する。

- noteは型や位置を中心にし、local値・Secret内容を含めない。
- move/borrow/freezeの提案を機械的に出さない。freezeできないresourceへfreezeを提案しない。
- 関数effectsを追加する案と、実行者の許可が必要な案を区別する。
- LSPのrelatedInformationとCLI/JSONが同じ元診断から生成される。

**検証：** List、closure capture、private factory、native接続、子task転送、generic callback。Unicode位置はCLI scalarとLSP UTF-16で同じ箇所を指す。redactionをバイパスしない。

### DIAG-02：checkpoint継続と外部境界

InvalidContinuation、begin制約、external内のawait/publish拒否を説明する。可能ならcheckpoint作成場所、現在の関数呼出し/scope、制約の種類を添える。

- 「revertは実行位置を戻さない」「resumeが必要な場面」を理由として説明する。
- 既に終わったscopeや別呼出しへ戻せないことを説明する。
- 実行中のblock枠を保持する既存revertを壊さない。
- Published/NotSent/Unknown等の意味を一つの汎用retry表示へ潰さない。

**検証：** 外側checkpointへif内からrevert、終了済scope、再帰の別呼出し、begin、active branch、external、publish部分適用。

### DIAG-03：子taskとcleanupの表示

既存TaskError/Failureを再利用し、原因tree、typed budget、wait graph、cleanup failureを見やすく表示する。asStdが情報を投影して失う契約を変えず、詳細表示のために元のFailureを選べるようにする。

**検証：** 取消、logical timeout、native work/steps/history超過、元診断付きFailed、待機循環、主失敗＋cleanup失敗。最大件数と省略表示を定義し、巨大な原因treeを無制限にformatしない。

## C. エディタの記録デバッガー

### DBG-01：記録viewのengineを分離する

現在のstdin command loopから、記録読込み、event位置、task選択、breakpoint、state/diffを呼ぶAPIを分離する。CLIも新engineを使う。

- 記録のcompiler/fingerprint/index長、署名、予算の検証は共通化する。
- stateを得るために元プログラムの外部処理を再実行しない。
- 全snapshotを複製せず、既存index/deltaの必要範囲を復元する。
- selected taskに対するstepと全体event位置を別に管理する。
- source-free記録とsource名の表示を維持する。

**検証：** 既存CLIのstep/back/line移動/continue/reverse-continue/break/task/checkpoint/state/files/diffの同等性。破損記録、過去のreadable範囲、署名違反、index不足、末尾/先頭の移動。

### DBG-02：boundedな値閲覧とwatch

- scope/variableをlazyに列挙する。List/Map/Bytesはoffsetと件数でページ化する。
- owner/cycle/型/長さ/省略を表示し、任意のgraphを全部文字列化しない。
- Secretと機密patternの既存redactionを適用する。
- watchの初期範囲は変数、field、検査済index等の読取り。function call、trait getter、callback、代入、await、publish、external、乱数/時刻観測を許可しない。
- object IDは「記録event/session内の表示ID」であり、永続的なphysical addressとはしない。
- event移動で無効になったreferenceへstale状態を返す。前eventの可変viewを新eventへ流用しない。

**検証：** 深い値、循環、大List/Map、Frozen、Secret、opaque resource、取消済task、checkpoint復元、範囲外watch。閲覧前後でruntime digest・外部操作数が同一。

### DBG-03：DAP transportと最小client

stdioのDebug Adapter Protocol連携を実装する案。command名候補は`rewind dap`だが、CLI-02でhelpと一緒に確定する。

| request | 初期提供案 |
|---|---|
| initialize / disconnect | capabilities、session解放 |
| launch | 検証したdebug traceを開く。初期版ではソースのlive起動ではない |
| setBreakpoints | 記録内の実行可能source/lineに設定、検証結果を返す |
| threads / stackTrace | VM taskとそのeventのstack。OS workerをVM threadと偽装しない |
| scopes / variables / evaluate | DBG-02のbounded閲覧と制限付きwatch |
| next / stepIn / continue | 記録を前へ移動。stepOutはframe情報で意味を検証できる場合に追加 |
| stepBack / reverseContinue | debug記録を後ろへ移動 |
| source | 利用可能なsourceだけ返す。source-freeで本文不在なら位置と不在を説明 |

compact記録にstep履歴を作ったふりをしない。unsupported capabilityと理由を示す。Content-Length、messageサイズ、JSON深さ、出力queue、session cacheに有限上限を設ける。ネットワークlistenerを初期transportへ追加しない。

**検証：** request/response ID、event順、切断、欠損/過大message、不明request、短い/長い記録、二つのclient session、Windows/Linuxのstdio。最小のエディタ設定例とCLI transcriptを両方提供する。

**保留：** live breakpoint、process attach、任意expression評価、外部作用の書換え、GUIの中断中編集。必要なら別の受入を追加する。

## D. 費用と保持理由のprofile

### PROF-01：仕事量の帰属

既存logical profileを拡張し、task/function/source単位のVM stepsとnative operation単位のworkを示す。inclusive/exclusiveの意味を明記し、合算で二重課金しない。

- 既定実行では収集を無効にし、重い集計・formatをsafe pointへ追加しない。
- profile有効でもscheduler選択、乱数、値、publishの順を変えない。
- wall clock測定は別項目とし、logical workをミリ秒として表示しない。
- textの上位項目とJSONの全bounded項目を用意する。

**検証：** nested call、再帰、generic specialization、async/await、native wrapper、取消とbudget失敗。同じ計算の非profile実行とdigest/出力/観測が一致。追加費用は測定し記録する。

### PROF-02：checkpointによる保持の説明

現在state、checkpoint、Task、観測/secret保護、Host resourceがrootになることを説明するprofileを追加する。

- 共有pageの参照総量と実際のunique量を区別する。
- checkpointごとの数値を足したものをRSSと表示しない。
- 「dropすれば必ずこの全bytesが解放」と断定せず、他rootとの共有を示す。
- 初期版はroot別の参照量と共有の有無から始め、exclusive保持量は正確に算出できる場合だけ表示する。
- 詳細retainer走査は明示した調査操作で行い、通常GCに常時追加しない。調査にもwork/memory上限を設ける。

**検証：** 同じpageを二つのcheckpointが保持、独立COW更新、begin、drop、Task完了、Bytes slice、native cleanup、循環、weak index。runtime全体のledgerと矛盾しない。

## 作業順と版の判断

CLI-01 → CLI-02 / DIAG-01 → DIAG-02 / DIAG-03 → DBG-01 → DBG-02 → DBG-03 → PROF-01 → PROF-02。

依存しない項目でも、まず一つの差分をreview可能にする。診断追加field・DAP capability・profile schemaの互換方針を各commitで明記する。計画段階ではlanguageを更新しない。実装完了と版公開は別に記録する。

## v2.1公開前の確認項目

- [ ] root/languageのmatrixと移行手順
- [ ] 所有権・効果・継続・task/cleanup診断の位置とredaction
- [ ] CLI debuggerの既存commandと記録形式の保証維持
- [ ] DAPの正常系・異常系、source-free、compactの明示的な拒否/制限
- [ ] watch/variablesで外部作用が発生しない
- [ ] logical費用・共有領域の表示と測定
- [ ] Windows/Linuxのローカル確認、正式公開に必要な検証記録
- [ ] HTML/PDF、API、sampleと実装の一致

このチェックリストは公開CI/CDの実行指示ではありません。現在の停止指示を守り、正式公開に必要な未実施検証は未実施として残します。
