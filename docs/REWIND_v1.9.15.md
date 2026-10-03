# REWIND v1.9.15

## native buffer を使う transform と疎行列

`std.fft.transform` は二つの rank-one FloatArray の複素 FFT、`convolve` は実数の線形畳み込みを提供する。transform の長さは同じ非ゼロの2の冪で最大2^20。正方向は exp(-2πikt/n)、逆方向は符号を反転し1/nで割る。Spectrum の real / imag は native 配列。畳み込みは output = left.len + right.len - 1 を次の2の冪へ padding し、同じ FFT 上限を使う。片方が空なら結果も空。Float64 の近似計算であり、整数の厳密な畳み込みではない。

FFT は bit reversal と反復 radix-2 の O(n log n)、O(n) native storage。twiddle は段ごとの位置から再計算し、長い連続乗算の誤差を避ける。同じ twiddle を複数 block で共有する。VM object を sample ごとに作らない。非有限の入力・中間値を拒否し、入力を変えずに結果を返す。

`std.sparse.create` は rows / cols と三つの COO native vector から CSR の Matrix を作る。行・列・entry は各最大2^20。COO は任意の順序でよく、同じ位置を stable な入力順で compensated sum し、exact zero を除く。offsets / sorted unique indices / values は native IntArray / FloatArray。O(nnz log nnz + rows) で構築し、dense 行列へ展開しない。

`matvec` は CSR の page を借用して順次走査し、各呼出しで nnz-sized な column / value buffer をコピーしない。入力 vector の indexed access と出力だけ O(cols + rows) の一時 storage を確保する。O(nnz + rows + cols) の検証・計算を行い、任意に構築した Matrix の不正 offset / column / shape、重複 column、非有限値を拒否する。strided な入力も descriptor の論理順に読む。

## norm と共役勾配法

`std.numeric.scale` は有限 scalar と配列の積を native buffer で作る。`norm2` は scaled Euclidean norm。巨大・微小な有限値をそのまま二乗して overflow / underflow することを避け、配列の page を allocation せずに走査する。empty の norm は0。最終 norm が Float64 に収まらなければ NumericOverflow。

`std.sparseAsync.conjugateGradient` は square な対称正定値行列を前提とする、zero initial guess の共役勾配法。正の有限 tolerance と1..100000の反復数を指定し、SolveResult の solution、iterations、residual、converged を返す。目標は `tolerance * max(1, ||b||)`。収束候補は recurrence の値だけで判断せず、実際の `b-Ax` を作って scaled norm を確認し、drift があれば direction を restart する。反復上限では最後の実残差と収束状態を返す。

非正の search denominator、二乗 search residual の underflow は NumericDomain。非常に不良な scaling、特異・不定値・非対称な行列に対する成功を保証しない。前処理や任意行列の solver を装わない。有限入力でも中間計算の overflow は Result の失敗となる。

反復の間で task.yieldNow を呼び、キャンセルを既存の TaskError として伝える。FFT / CSR 構築 / 各 matvec / vector kernel は同期的であり、個々の kernel の内部まで協調的な分割を保証しない。no host worker、external effect や観測は不要。長い kernel の分割は引き続き改善する。

## 保持と予算

既存 numeric page / tree の COW と Runtime ledger を使い、checkpoint / revert / branch / Task / freeze / source-free replay と整合する。結果の page は保持された root の寿命に従って解放する。kernel は native work と出力・scratch の保守的な memory admission を先に検査する。Result へ fatal work / history budget / cancellation を隠さない。大きい配列に必要な budget は用途に合わせて CLI で指定する。

## profile の GC / numeric 指標

`rewind profile main.rwc` は GC の attempts / completed / failed、duration_nanos、reclaimed_objects、work と、numeric page の live_bytes / 累積 allocated_bytes / registration_visits を追加で報告する。heap の mark / sweep / budget rollback にかかった host wall time であり、VM 側の root 集合構築・compiler・native kernel 全体の時間ではない。計測は profile のときだけ有効。値は checkpoint、言語からの観測、state digest と trace 照合へ含めない。失敗した GC の計測でも root / sweep の atomicity は維持する。累積 allocation と、解放後の live byte を区別する。

## 検証・測定

独立 DFT、直接畳み込み、既知の sparse product と解を比較する。重複・ゼロ、rank / shape / column / offset の不正、NaN、finite scale、巨大 / 微小 norm、checkpoint 復元、language gate、native work、反復不足、タスクとキャンセルを検証する。`examples/sparse-fft` を両 OS の展開済み SDK で compile し、ソースを削除した実行と compact replay を行う。debug replay も回帰テストで検証する。

`python3 scripts/benchmark-transforms.py target/release/rewind --output transforms.json` は native な一定入力に FFT と inverse を繰返し、一つの checkpoint を保持する。初期化・検証・GC を含む source-free 実行を、warm-up 後に3回測り、elapsed、peak RSS、profile の storage / GC 指標を記録する。compile 時間は除く。whole program の指標であり、kernel 単体の latency や OS window の応答時間ではない。

reverse-mode の勾配・optimizer・モデルの保存、GUI / 通信 / DB の統合、追加 GUI、HTTP server / TCP、協調的 kernel と v2.0 の到達条件は引き続き実装する。
