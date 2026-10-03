# REWIND v1.9.11

## 共有される数値 storage の予算

language 1.9.11 以降は、FloatArray / IntArray の同じ native page と tree node を Runtime の会計で一度だけ数える。shape / stride descriptor、VM の container と checkpoint の metadata、scratch の費用は別に残す。これまでは実際の buffer が共有されていても、別の値・heap root・task context から同じ buffer の費用を重複計上し、変更が少ない checkpoint や大きい数値 task が予算で止まることがあった。

COW の書換えは新しい page と path の分だけ加算する。最後の参照がなくなるとその分を即時に減算する。current state、checkpoint / branch、終了 task の結果、channel 等がまだ参照する値は保持する。VM heap の循環は既存の mark / sweep で到達性を判定してから native 参照を解放する。checkpoint に保持された値を GC が勝手に捨てることはない。

会計の registry は page を強参照しない。弱い登録と Runtime ごとの再利用しない ID を使い、COW clone / Drop で差分を更新する。登録済みの tree は再走査せず、List / Map は scalar-only subtree と登録済みの persistent path を省く。値の書換えではその memo を無効化する。会計の登録は VM thread で行い、別 Runtime の予算は独立する。COW clone は既存の content hash を引き継ぎ、書換え前の page を余分に hash しない。

これは RSS そのものの上限ではなく、保守的な admission units の改善。新しい Node / registry metadata を含む native storage の事前見積りも更新する。Rust の embedding で Runtime に渡した配列を別の owner が持ち続ける場合、その登録済み storage は owner が捨てるまで費用が残る。VM 内でも user が保持する値と checkpoint は保持対象であり、上限を自動的に増やさない。

GC の4MiB trigger は、数値 descriptor の複製を新しい buffer 全体の確保として数えず、新しく確保・登録した native node の累積量を使う。256 heap allocation の trigger は維持する。大きい新規 buffer と短命な COW page は対象に含め、共有するだけの引数・結果で不要な GC を繰り返さない。予算検査と GC trigger は別であり、GC で user の checkpoint を破棄しない。

## scheduler との統合

scheduler の16MiB上限は task context / result / queue 等の metadata と非数値 payload の費用を引き続き制限する。共有する数値 page は Runtime の履歴 memory 予算に統合する。scheduler が新しい数値 root を登録した場合も、VM を進める前に native 会計の増加を検査する。会計の増加がない命令で追加の全履歴検査を行わない。

language 1.9.10 以前は従来の会計を維持する。数値 API、wire format、content digest、数値の計算順序、external の再送禁止、既存 checkpoint の動作を変えない。native_work / step / history 予算の改変は従来どおり replay で拒否する。

## 検証と配布

100万要素の共有・繰り返す登録・COW・別会計・最後の owner の解放を検証する。128 checkpoint と差分、入れ子の List / Map、container の unique な書換え、予算失敗の rollback、checkpoint に保持された循環 heap とその解放を検証する。

CLI では100万要素の内積を task に渡し、64 checkpoint と COW を小さい履歴予算で実行する。source-free artifact と compact replay で値と予算を照合し、小さすぎる予算の拒否も確認する。SDK の numeric-memory 例はこれらを一つのプログラムで実行する。

速度 / foreground handoff の測定は scripts/benchmark-numeric-cooperation.py、繰り返す更新と RSS の測定は scripts/benchmark-numeric-memory.py を使う。測定値はその workload の結果であり、一般の UI 待ち時間や GC pause の保証ではない。

一般の List / Map root と履歴 metadata の重複費用、累積 task quota / profile、観測 export、host snapshot の一括 materialization、残る native kernel の協調化と GUI / 通信 / 数値の統合は引き続き監査する。この版で v1.9 全工程や v2.0 の到達を宣言しない。
