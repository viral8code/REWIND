# DIST-01/02・MEM-06/07 現状調査

構想の「まず既存基盤を調べる」段階を実施した記録です。コードの変更はありません。結論は「いま実装を足す必要は小さく、次に判断する材料はこれ」です。

## DIST-01：実行時依存の実測（Linux）

`target/release/rewind`（約30MB、静的リンクのRust実行ファイル）の動的依存は次の4つだけです。

| 依存 | 備考 |
|---|---|
| `libgcc_s.so.1` | unwind |
| `libm.so.6` / `libc.so.6` | glibc |
| `ld-linux-x86-64.so.2` | loader |

- 最低glibcは実測で**2.35**（`objdump -T`の最大symbol version）。release build情報（BUILD_INFO.json）と一致する前提で記録します。
- JVM・Rust・Python・追加の共有libraryは不要です。`.rwc`はsourceなしで`rewind`が実行します（既存の保証）。
- つまり「最小runtime」は、いまのSDKから`share/`（文書・例・std source）を除いた`bin/rewind`単体で成立する見込みです。別バイナリに分離する利点は、サイズ（約30MB）が既に小さいためほぼありません。

**判断：** 別runtime配布物は作りません。必要になった場合の候補は「`bin/rewind` + 公開鍵 + SHA256SUMS + 依存license一覧」を1つのtar.gzにするだけで、新しいcompile経路は不要です。未実施：clean環境（glibc 2.35未満・muslなど）での起動確認と、`.rwc`だけを置いた環境での起動前診断の整理。`rewind compat`（DIST-03）が互換不一致の拒否を既に担っています。

## DIST-02：Windows確認範囲

CIはWindows Server 2022で、回帰テストと配布物の署名・展開・入力・record/replay・source-free実行を確認しています。確認済みと未確認を分けます。

| 項目 | 状態 |
|---|---|
| zip展開だけで`rewind.exe`が動く | CIで確認済み |
| source / `.rwc` / record・replay | CIで確認済み |
| 空白・Unicode path | テストにfixtureあり（実機の対話環境は未確認） |
| 長いpath（260文字超） | 未確認 |
| console encoding（CP932など） | 未確認 |
| GUI（Win32） | CIでfixtureのみ。実デスクトップ表示は未確認 |
| HTTPS/TLS証明書、PostgreSQL | 実Windowsでの接続は未確認 |

**判断：** 「対応済み」と書ける範囲は上の確認済み行だけです。未確認の行は、ローカルのWindows環境で手動確認するまで対応済みと書きません。確認用の手順書は、実機で確認できる担当者が決まってから作ります。

## MEM-06：scratch bufferの現状

- 数値処理（`src/v2/v092/numeric.rs`）はoperationごとにscratch量を見積もり（`scratch_estimate`）、VM予算に対して**事前に確保可否を判定**してから割り当てます。コメント「All scratch and output allocation has a preflight against the complete VM budget」のとおりです。
- QR・固有値などはchunk単位の作業で、`order_scratch`のように作業状態自体を値として持ち、step間で再利用します。
- 汎用のscratch arena（task間で使い回す共有buffer）は存在しません。各operationが確保して終われば解放します。

**判断：** 汎用arenaは追加しません。理由は、(1) 既存の事前確保判定が予算超過を防いでいる、(2) 使い回しはtask隔離・Secret消去・取消後の未初期化bufferという新しい失敗要因を持ち込む、(3) 削減したいallocationの具体的な計測がまだない（PERF担当の計測待ち）。PERF側でallocationが支配的と示されたcaseが出てから、そのcaseだけに限定した再利用を検討します。

## MEM-07：資源会計の現状

- `src/shared_payload.rs`のLedgerは、共有payload（Arc）を**一度だけ**数える仕組みです。同じ値を複数のviewが参照しても重複課金しません（`register_value`/`register_bytes`、世代付きの`prune_bytes`）。
- 数値配列・text・map・storageは各`*_storage.rs`が別々のcounterを持ち、VMの履歴memory予算に合算されます。
- 外部確保（mmap、native library内部buffer、device memory）の種類別counterはありません。外部確保自体が現状存在しないためです。

**判断：** 種類別counterは、外部確保を導入する時（MEM-01/05/08）に同時に設計します。先に作ると使い道のない抽象になります。導入時の条件は構想どおり「確保前に予約、失敗で戻す、close後もnative callが使用中なら実解放まで費用を残す、viewごとに重複課金しない」です。

## 次に実装へ進む場合の入口

1. MEM-01/03/04（Arena・freeze/copy・loan）は、**FFIまたはmmapの具体例が決まってから**同時に設計する（単独では価値が出ない）。
2. EXT-01/MEM-08は対象のC libraryとOSを先に決める必要がある（確認事項として利用者に質問する）。
3. DIST-01/02は、新しい実装よりも**実環境での確認**が残件。
