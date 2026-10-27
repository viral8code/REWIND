## 第20章 大きな整数・十進数・日時

### 失敗を取り出す補助関数

この章と次の章の標準ライブラリは、ほとんどの関数が `Result` を返します。例を短くするために、`Ok` なら中身を取り出し、`Err` なら止まる補助関数を使います。

```
fn take<T,E>(value: Result<T,E>) -> T effects {} {
    match move value {
        Ok(item) => { return move item; },
        Err(_) => { panic("失敗しました"); }
    }
}
```

実際のアプリケーションでは、`Err` を `match` や `?` で、ちゃんと扱ってください。止まってよいのは、例やテストのように、失敗が「プログラムの誤り」を意味するときだけです。

### BigInt: 桁数に上限のない整数

`Int` は 64 ビットで、それを超える計算は止まります。`std.bigint` の `BigInt` は、`Float` を経由せずに、正確な整数を扱えます。

```rewind
import std.bigint as big;

fn take<T,E>(value: Result<T,E>) -> T effects {} {
    match move value {
        Ok(item) => { return move item; },
        Err(_) => { panic("失敗しました"); }
    }
}

fn main()->Int {
    let two = take(big.fromInt(2));
    let value = take(big.pow(&two, 100));
    Out.println(take(big.format(&value, 10)));
    return 0;
}
```

```text
1267650600228229401496703205376
```

`BigInt` も無限のメモリを使えるわけではなく、容量と処理量の予算があります。巨大なべき乗の結果ではなく、余りだけが必要なときは、`std.modular` の剰余演算の方が適しています。

### Decimal: 十進数で正確に計算する

`Float` は二進数で値を持つので、`0.1 + 0.2` が `0.3` にならないように、十進の小数を正確には表せません。金額のように十進の桁が意味を持つ計算には、`std.decimal` の `Decimal` を使います。

丸め方は、計算のたびに **明示します**。`DecimalRounding::Exact` は、丸めが要らないときだけ成功する指定です。`HalfEven` は、ちょうど真ん中の値を偶数の側へ丸めます。

```rewind
import std.decimal as decimal;

fn take<T,E>(value: Result<T,E>) -> T effects {} {
    match move value {
        Ok(item) => { return move item; },
        Err(_) => { panic("失敗しました"); }
    }
}

fn main()->Int {
    let a = take(decimal.parse("0.1"));
    let b = take(decimal.parse("0.2"));
    let sum = take(decimal.add(&a, &b, 28, DecimalRounding::Exact));
    Out.println(take(decimal.format(&sum)));

    let tie = take(decimal.parse("2.5"));
    let rounded = take(decimal.quantize(&tie, 0, 28, DecimalRounding::HalfEven));
    Out.println(take(decimal.format(&rounded)));
    return 0;
}
```

```text
0.3
2
```

金額の計算では、丸めの規則を最初に決めてから実装してください。データベースとやりとりするときの精度の違いは、`std.dbDecimal` が扱います。

### 日時

日時の扱いには、時点を表す `Instant` と、時間の差を表す `Duration` が出てきます。変換や計算は、純粋な関数で、`Result` を返します。現在の時刻を観測する `clock.now` は、効果 `external` と `clock` が必要です。「日時を整形する」ことと「現在の時刻を読む」ことは、効果が違います。

タイムゾーンは、必ず名前で指定します。夏時間が終わる日には、同じ現地時刻が 2 回あり、`Earlier`（早い方）、`Later`（遅い方）、`Reject`（エラーにする）のどれかを選ぶ必要があります。次の例は、2024 年 11 月 3 日の米国東部時間 1 時 30 分を、早い方と遅い方の両方で解決して、その差を調べます。

```rewind
import std.datetime as datetime;

fn take<T,E>(value: Result<T,E>) -> T effects {} {
    match move value {
        Ok(item) => { return move item; },
        Err(_) => { panic("失敗しました"); }
    }
}

fn main()->Int {
    let wall = datetime.Calendar(2024, 11, 3, 1, 30, 0, 0);
    let first = take(datetime.resolve(move wall, "America/New_York", datetime.Ambiguity::Earlier));
    let second = take(datetime.resolve(datetime.Calendar(2024, 11, 3, 1, 30, 0, 0), "America/New_York", datetime.Ambiguity::Later));
    let gap = take(datetime.difference(&second, &first));
    Out.println(take(datetime.durationSeconds(&gap)));
    Out.println(take(datetime.format(&first)));
    Out.println(take(datetime.format(&second)));
    let tokyo = take(datetime.calendar(&first, "Asia/Tokyo"));
    Out.println(tokyo.offsetSeconds);
    return 0;
}
```

```text
3600
2024-11-03T05:30:00Z
2024-11-03T06:30:00Z
32400
```

同じ現地時刻の「1 時 30 分」でも、2 つの `Instant` は 1 時間（3600 秒）離れています。`Instant` を日本時間に直すと、UTC との差は 9 時間（32400 秒）です。

存在しない現地時刻（夏時間の始まりで飛ばされる時刻）は、エラーになります。年月日を入れれば、どのタイムゾーンでも一意に `Instant` が決まるわけではない、という点を覚えておいてください。

IANA のタイムゾーンデータの版は、`datetime.databaseVersion` で調べられます。データベースに日時を保存するときは、保存先の精度で丸められるか、タイムゾーンの情報をどこに残すかを、あらかじめ決めてください。
