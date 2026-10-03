# REWIND v1.9.3 — scheduler の到達可能性と GC root の借用

## 回収対象

1.9.3 以降は、新しい scheduler identity が 64 個作られた時、または heap collection が必要な時に、VM safe point で scheduler の到達可能性を検査する。task / channel / group の上限 4,096 を累計作成個数だけで使い切らず、不要になった object を回収して再び作成できる。identity の番号は collection によって再利用しない。

global / local scope、stack、frame、defer、branch、現在の compute roots にある handle と、heap / collection / closure / Option / Result に含まれる handle をたどる。channel の queue、group の member、task の引数・結果・待機先・実行 context もたどる。

main / active task、実行中・待機中の task、未観測かつ無視されていない失敗を残す。ユーザーが保持する完了 task の結果も残す。到達不能な cold task、完了 task、channel / group と、それらの循環を回収する。未処理の失敗を消してプログラムを成功させない。

checkpoint は独立した scheduler / heap を持つ。current scheduler からの削除で保存済みの task を失わない。queued handle、using TaskGroup の cleanup、cold checkpoint の復元と repeated await の契約も保つ。旧言語版は従来の scheduler 保持規則を使用する。

## 費用・heap root

mark と sweep は native work と実行 work を消費し、予算内で解析できない場合は object を削除しない。child 数の事前検査で広い collection の scratch 展開を制限する。

heap collector に借用した root を渡す経路を追加する。VM の root を集める際、大きい String 等を Value として深く clone しない。埋込み用 Runtime.collect_heap は継続し、collect_heap_refs は同じ safe-point / external-root 契約で root を借用する。

scheduler の保持する結果と queue が不要になれば、その buffer の Arc 参照も解放する。保存された checkpoint が最後の所有者であれば、その破棄まで解放しない。これを物理参照の Weak でも検証する。

## 検証・継続事項

5,000 sequential tasks、1,500 channel の作成・送受信・close、100 group を、一つの実行で完了する。queued task handle と checkpoint を collection をまたいで復元し、source を除いた artifact / trace replay を検証する。未観測 failure を残す。実 buffer の解放、checkpoint の保持、到達不能な channel / cold task の循環、予算失敗の非破壊性も検証する。

この版で Runtime 全体の RAM が bounded になったとは扱わない。累積の task instruction quota / profiling metadata、観測・trace、host file snapshot、共有 buffer の admission は別に監査する。task budget を revert でリセットするために過去の quota を捨てることはしない。native kernel の公平性・キャンセル、GUI・アルゴリズムと v2.0 統合条件も継続する。
