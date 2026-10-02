# 型付き数値配列

REWIND 1.8.0 で `rewind run main.rw`。追加の外部作用は不要。

部分 pivot LU により `[[0,2],[1,3]] × x = [4,7]` を解き、`1` と `2` を出力する。その後の配列更新を publish し、checkpoint へ revert すると、公開済みの `99` に続いて元の `0` を出力する。更新は変更 page への COW であり、checkpoint の配列を壊さない。

`withFloat` は新しい値を返すため、更新結果を変数へ代入する。各関数は Result を返す。この例の take は失敗時に panic するが、アプリケーションでは StdError.code を match して処理する。[数値 API の契約](../../docs/REWIND_v1.8.md)を参照。
