# REWIND 3.6.0

## 言語

- **名前付き引数とdefault引数**（LANG-01）：`connect("host", secure: true)`のように`name: value`で引数を渡せます。関数宣言は`fn f(a: Int, b: Int = 80)`のようにliteral（数値・String・Bool・`None`・負数）のdefaultを持てます。defaultは後ろのparameterにだけ置けます。呼出しは検査前に位置引数へ書き換わるため、型・所有権・moveの規則は位置引数と同じで、**評価順は書いた順ではなくparameter順**です。対象は宣言済みの関数とmodule関数で、method・closure・関数値には使えません。parameter名は公開APIの一部になり、API baselineに記録されるため、改名は`api-diff`で互換性のない変更になります。

## 追加（library）

- **`std.table`**（DATA-02/03）：Int/Float/Text/Bool列とNULL maskを持つ不変table。列の選択・改名・削除はデータを共有し、head/slice/batch/where/takeRows、mask・map・cast、`aggregate`（group別のCount/Sum/Min/Max/Mean）、出力上限つき`join`（Inner/Left）を提供します。numeric kernelへは`floatArray`/`intArray`で渡します。
- **`std.layout`**（MEM-02）：整数7種・配列・little/big endian・C規則のpaddingまたはpackedを明示するbinary layout。Pythonの`struct`と一致することをテストで確認しています。
- **`std.result` / `std.option`の`*Owned`**（LANG-05）：非Shareのowner（List、接続など）を複製せずに一度だけ次へ渡す`mapOwned`・`thenOwned`・`mapErrorOwned`。

## 追加（tool）

- **`rewind profile`のcoverage**（QUAL-03）：実行されなかった行を行番号で出します（JSONは`coverage`、textは`coverage FILE: n/m lines`）。branch coverageではありません。

## 互換性

parameter名をAPI baselineへ記録するようにしたため、`api-diff`は公開関数のparameter改名を非互換として扱います。

詳細は[言語構想](../archive/v2-next-language.md)・[データ構想](../archive/v2-next-data.md)・[memory構想](../archive/v2-next-memory.md)を参照してください。
