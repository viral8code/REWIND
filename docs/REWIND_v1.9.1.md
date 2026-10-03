# REWIND v1.9.1 — 外部観測の private spill

## 保持と再利用

完了した外部操作の JSON 結果を、immutable な参照共有 Segment に保持する。既存の journal / file page と同じ historyMemory / historyStorage / spillThreshold 方針で一時ファイルへ退避する。checkpoint は操作 cursor を保持し、結果自体は巻き戻さない。begin からの再実行に必要な記録を削除して RAM を減らす方式ではない。

同期操作と非同期の HTTP / DB 操作の両方に適用する。読み直しは記録結果だけを返し、host に再送しない。result ごとの長さと SHA-256 を記憶し、退避ファイルの破損・切断・肥大化を拒否する。読み直しの buffer は記録長で制限する。秘密の検査・trace 書出しの拒否条件は従来通り適用する。

spill file は Unix で owner read/write の 0600 とし、Windows ではユーザーの temp directory の ACL を継承する。最後の所有者が失われた時に削除する。暗号化や crash 後の回収・復旧を保証する契約ではない。

## 予算と失敗

送信前の最大結果 buffer 予約と memory admission は維持する。pending buffer、結果が記録できなかった操作、operation / completion metadata も予算に含める。完了後の payload は Segment の memory / disk usage に数え、resident metadata と二重に数えない。

退避先容量・I/O の失敗は黙って無視しない。host が既に完了して記録結果を保持している場合、その既知結果を残して失敗を報告する。予算を増やした後もその操作を自動再送せず、既知結果を再利用する。結果を記録できなかった操作の OutcomeUnknown と自動再送禁止も維持する。

一つの外部結果の上限 16 MiB と entry 上限 1,000,000 は継続する。metadata と pending buffer まで RAM 無制限にはしない。historyStorage は明示的な累積上限であり、長時間実行では必要な容量を設定する。

## trace と wire

Entry の wire は fingerprint / outcome の JSON string / reservation / pending を維持し、既存 trace を読める。file path やメモリ上の Segment ID を公開 artifact に入れない。

この版は Runtime の resident payload を減らす変更である。export は引き続き DOM を構築する。外部結果の trace 上限 16 MiB は disk 上の payload も数えて判定し、resident metadata の少なさで上限を迂回しない。大きい trace の streaming writer / reader は後続監査で扱う。

## 測定・検証

64 個の 60 KiB 結果を historyMemory 128 KiB の Runtime で保持し、checkpoint / revert と import / replay で host を再実行せず取り出す。disk 失敗後の既知結果、16 MiB trace 上限、破損検出・退避ファイルの権限と解放も検証する。実 HTTP chunk download を強制 spill し、close / revert 後の記録再利用を検証する。

前版の GC 圧力の測定手順を `scripts/benchmark-gc-pressure.py` に追加した。Linux の最適化ビルドで 512 KiB の一時 List payload を 200 回作る 3 回の測定では、v1.8.8 / v1.9.0 の median peak RSS は 222,508 / 20,160 KiB、elapsed は 0.245 / 0.039 秒だった。この一つの workload の結果であり、全プログラムの速度・メモリ改善率を保証しない。

bounded file I/O、trace streaming、共有 buffer accounting、task / native の公平性、GUI と残るアルゴリズムの実例を継続する。v1.9 全体・v2.0 の到達条件は未完了。
