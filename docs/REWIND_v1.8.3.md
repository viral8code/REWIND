# REWIND v1.8.3 — Decimal と DB の exact 数値変換

`std.decimal` と `std.dbDecimal` を追加する。Decimal は native BigInt の coefficient と signed scale を持つ不変な値で、数値は coefficient × 10^(-scale)。入力・算術・丸め・DB wire に Float を介在させない。

## 型・精度・丸め

直接 constructor を公開せず、parse / fromCoefficient で作る。parse は ASCII の符号・十進数字・小数点・任意の e/E exponent。空白、separator、NaN / Infinity は拒否する。coefficient は最大10,000桁、scale は -10,000..10,000、入力は最大20,032 byte。format は plain な十進文字列。representation は非負 scale を plain、負 scale を coefficient の e 表記にし、scale を保ったまま再読込できる。coefficient / scale は明示的に照会できる。

add / subtract / multiply は precision（1..10,000）と DecimalRounding を明示する。precision は結果の coefficient の桁数で、不要な桁を落とす場合は scale も変える。carry による桁増加も扱う。quantize は target scale、divide は target scale と precision を指定し、scale を勝手に変えて収めない。指定 scale では precision に収まらない場合は DecimalPrecision。

| DecimalRounding | 非ゼロの破棄部分の扱い |
| --- | --- |
| TowardZero / AwayFromZero | ゼロ方向 / ゼロから遠い方向 |
| Floor / Ceiling | 負の無限大 / 正の無限大方向 |
| HalfUp / HalfDown | 最も近い値、tie は遠い方向 / ゼロ方向 |
| HalfEven | 最も近い値、tie は偶数 |
| Exact | 破棄する非ゼロ部分があれば DecimalInexact |

0 除算は DecimalDivisionByZero。Syntax / Size / Scale / Precision / Domain も StdError の Decimal prefix code として返す。VM の native work / memory 上限は Result より先に処理を停止する場合がある。

## snapshot・比較・保存

数値比較の compare / scalar 比較演算子 / Eq / Ord / Hash は `1.0` と `1.00` を同じ数値として扱う。VM の保存・merge 用比較は scale も含め、表示や quantize の状態を失わない。Map key は精度・scale 範囲内で一意に正規化し、同じ数値を二重登録しない。keys の scale は入力の literal scale を保持する契約ではない。

native 値・canonical key は Arc で共有し、commit / revert、freeze / thaw、Task、source-free artifact / replay に対応する。native payload と canonical cache を予算に含め、最後の参照の破棄で解放する。診断の cached numeric SHA-256 と scale により、全桁を毎回展開せず VM 状態の変更を検出する。内部 wire は canonical coefficient の hex と scale の pair。復元時に容量・scale を検査し、非 canonical な Map key は拒否する。

明示的な演算前に scale alignment / division / rounding の一時領域と仕事量を計上する。数値比較にも入力サイズの費用を計上する。一般の Decimal 四則に隠れた global rounding context を作らない。

## JSON

toJson / fromJson は representation の Json::Text を使う。通常の JSON number parser に Decimal を暗黙挿入しない。fromJson は string 以外を DecimalJsonType とする。負の scale と大きな exponent も Float に落とさず roundtrip する。

## PostgreSQL / SQLite

`std.dbDecimal.parameter(&value)` は NUMERIC 用の DbValue::Text を作る。PostgreSQL は server の parameter type が NUMERIC の場合だけ strict な finite Decimal text を binary NUMERIC に変換する。Int / Float を勝手に NUMERIC に変換しない。結果の NUMERIC は exact な Text として返し、`std.dbDecimal.cell(row,index)` が Decimal に復元する。既存の DbValue enum を拡張せず、旧 module の exhaustive match / API を維持する。

wire の digit、weight、sign、dscale、長さ・容量を検査し、NaN / Infinity と dscale に隠れた非ゼロの桁を拒否する。DB に送る・受取る expanded numeric representation は10,000桁まで、dscale は10,000まで。VM だけで扱える大きな sparse exponent が DB の容量範囲外の場合は DbType / capacity error になり得る。DB の dscale に従うため、負の VM scale を DB が同じ metadata で返す保証はしない。

SQLite の numeric affinity は文字列を Float に変換し得る。`std.dbDecimal.sqliteParameter(&value)` は representation の UTF-8 BLOB を作り、この自動変換を避ける。NUMERIC affinity の列でも丸めず保存・復元できる。SQLite の通常の SQL 算術が BLOB を exact Decimal として計算する契約ではない。計算は VM の Decimal、または PostgreSQL NUMERIC で行う。

cell は Text / UTF-8 Bytes を strict に解析し、Null は DbNull、他の型は DbType、範囲外は DbColumnRange。DB transaction と VM commit / revert は従来どおり独立する。値を非公開 parameter として登録する場合は、既存の Secret<DbValue> / privateParameter を使う。

## 検証・後続

[数値例](../examples/decimal/main.rw)、[PostgreSQL](../examples/decimal-postgres/main.rw)、[SQLite](../examples/decimal-sqlite/main.rw)。受入は0.1+0.2の exact 計算、signed rounding / tie / carry、scale・精度限界、VM と Map の比較、wire 拒否、publish後のrevert、source-free replay、実 NUMERIC と SQLite affinity、展開済み両 OS の SDK を含む。

本版で1.8工程全体を完了とはしない。次に日時 / IANA / DST、Unicode / regex、増分 JSON / CSV を進める。[到達計画](v2-design.md)の1.9・2.0の範囲も維持する。
