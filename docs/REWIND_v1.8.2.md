# REWIND v1.8.2 — exact integers

v1.8 の数値型工程を分け、先に immutable な native BigInt と `std.bigint` を追加する。既存の Int は checked な64 bitのまま。BigInt は constructor や暗黙の Int / Float 変換を持たず、parse / fromInt と標準関数で作る。

## 算術・表現

parse / format の radix は2..36。符号は任意の + / -、空文字・空白・separator・radix prefix は拒否する。fromInt は正確、toInt は範囲外で BigIntOverflow。add、subtract、multiply、divide、remainder、modulo、gcd、pow、modPow、compare、bits、negate、bitAnd / Or / Xor / Not、shift を提供する。

divide はゼロ方向への切捨て、remainder は被除数と同じ符号。modulo は正の modulus に対し0以上 modulus 未満。gcd は非負。modPow は非負の整数 exponent と正の modulus、pow は0..2^32-1の exponent（0^0 は1）。negative shift は拒否し、右 shift は算術 shift。bit 演算は負数を含む無限二の補数として扱う。

値は最大262,144 bit、parse 入力は最大262,144 UTF-8 byte。Size、Syntax、Radix、DivisionByZero、Domain、Overflow は BigInt prefix の StdError。VM の native work / memory 上限は型の最大値より前に拒否する場合がある。演算前に入力と最大出力に基づく仕事量・scratch を確認し、CPU 演算は Rust の num-bigint で行う。small addition を multiplication の二乗費用にしない。

## checkpoint・型・保存

Rust Arc の immutable な整数を VM 値として共有する。変更は新しい値を返し、commit / revert、freeze / thaw、Task の受渡しに従う。最後の参照がなくなると native allocation を解放する。演算後の余剰 limb capacity を compact にしてから保持する。保持量は VM の予算に含め、snapshot 間の実共有量の精密化は1.9の監査に残す。Map の primitive key と Eq / Ord / Hash / Display に対応し、符号付き数値順で比較する。digest は符号と magnitude の cached SHA-256、wire は canonical な signed lowercase hex で、復元時に容量と canonical form を検査する。

## JSON と後続

toJson / fromJson の公開表現は decimal な Json::Text。JSON の数値 token を一旦 Float にする変換は使わない。toJson に対応する fromJson は string のみ受け付け、他は BigIntJsonType。これは相互運用上の明示的な文字列契約で、通常の JSON parser の既存仕様を変更しない。

[例](../examples/bigint/main.rw) は2^100の計算、publish後のrevert、JSON、Mapを示す。受入は signed division の恒等式、大整数参照値、modPow、bit / shift、capacity / malformed wire、source-free run / replay、所有権・Task・Map・native budgetを検証する。両 OS の SDK 成功後に main 統合・Release 公開を行う。

次の patch で Decimal の precision / scale / rounding と DB / JSON の exact 変換を実装する。日時 / IANA / DST、Unicode / regex、増分 JSON / CSV も残り、[到達計画](v2-design.md)は維持する。本版を1.8工程全体の完了とはしない。
