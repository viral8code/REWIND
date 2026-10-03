# REWIND v1.9.17

## Module の global scope

1.9.17 では関数の型検査に declaration origin の global binding と名前修飾済み import binding を渡す。import 先の関数は entry file の無関係な変数を裸の名前で capture できない。private な module const と明示した dependency const は引き続き参照できる。import 内の local と entry の同名 variable に出ていた不要な shadow warning を解消し、同じ lexical scope の外側を隠す場合の warning は維持する。

1.9.16 以前の language mode では従来の scope 判定を維持する。新しい lexical rule を使う project は language を1.9.17に更新し lock / artifact を作り直す。source-free artifact の検証は embedded language / IR と現行 compiler に従う。

## Optimizer の native pass

SGD と Adam の公開 signature・所有権・typed failure の atomicity・checkpoint 契約は維持する。parameter / gradient / moment の native page を借用し、SGD は一つの更新結果、Adam は更新後 weight と二つの moment のみを作る。formula の項ごとの一時 FloatArray / page tree を省く。preflight は出力 pages と一時 native buffer を含め、計算前に native work と memory を検査する。

Adam は second moment の gradient を先に sqrt(1-beta2) で scale してから二乗し、bias correction も sqrt(moment)/sqrt(correction) の順に行う。weighted moment と更新値が有限に収まる場合、不要な g*g / variance/correction の overflow を避ける。correction は前版同様 expM1 で計算する。Float64 の近似計算であり、別版の実装との bitwise 一致を要求しない。

内部の optimizer primitive は既存 optimizer が使える language1.9.16でも利用できる。一般の言語演算子の rounding を変更しない。raw な負の variance、異なる shape、非有限値、configuration、非表現可能な中間・出力を拒否する。typed error では入力を変更しない。

## 検証

private / transitive const、entry global capture の拒否、不要 / 正当な shadow warning、旧 language mode、source-free と replay を確認する。既知の二回の Adam step、巨大な有限 gradient、typed error と input の維持、checkpoint を独立な期待値と比較する。前版の有限差分・学習・model codec / file-free replay の回帰を維持する。

`scripts/benchmark-model-training.py --optimizer sgd|adam` は同じ source-free workload を旧 / 新 binary で測り、elapsed、peak RSS、GC、累積確保と終了時保持量を区別する。tape の作成と勾配計算、初期化と検証も含む whole-program の測定であり、単一 native kernel の時間ではない。

GUI / DB / 通信の統合、HTTP server / TCP、長い kernel の協調動作と v2.0 の到達条件は継続する。
