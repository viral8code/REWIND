# 宣言的JSON schemaと型付きdecoder

更新日：2026-10-12。対象：codex/develop。**source実装済み・実行検証省略**。

[DATA-01](v2-next-data.md)のJSON検証、明示mapper、header付きCSV row adapterをstd.schemaへ実装した。既存std.configの入力優先順位とflatな設定検証は維持する。DB/form向けadapter、Decimal・日時向けrule、reflection/deriveは後続の範囲。

## Schemaの構築

any、boolean、number、integer(min,max)、text(min,max)、nullable、array、field、objectを提供する。numberはJsonのInt/Floatをそのまま受け、暗黙にFloatへ変換しない。textの長さはUnicode scalar。arrayは0〜65,536件の範囲を宣言する。

fieldは名前、子Schema、required、Option<Json>のfallbackを受ける。名前は1〜128 scalar。requiredとdefaultの同時指定はRequiredDefaultで拒否する。defaultはfield構築時に子Schemaで検証・正規化して保持し、不正ならInvalidDefault。NULLは欠損とは別の値であり、nullableを指定した場合だけ通常の型検証を省略して通る。anyはNULLを含む任意Jsonを受ける。

objectのUnknown policyはReject / Keep / Drop。fieldは宣言順で最大128件、一意の名前。Schema全体は1,024 node、深さ32まで。内部は有限なnode表と整数の子参照で表し、schemaの合成時に参照を移し替える。private constructorにより循環参照を作る入口は提供しない。

## 検証と結果

validateはdefault挿入とunknown policyを反映したJsonを返す。入力は変更しない。必須項目が欠損すればMissingField、明示NULLはdefaultで置き換えず、schemaに従って検査する。

エラーは最初の一件をValidationError(code,path,expected,actual)として返す。pathはField(String) / Index(Int)のFrozen列。実値、未知fieldの入力名、JSONの抜粋をerrorへ自動追加しない。expectedは型・制約の説明、actualはkindまたはMissing。Schema構築時のSchemaError.fieldは利用者が宣言したfield名を扱う。

objectではunknown policyを先に検査し、宣言済みfieldは宣言順、配列はindex順で進む。Unknown::Rejectのerrorは対象objectのpathを返す。全errorの収集や大量の連鎖診断は行わない。

一回のvalidateは最大65,536 validation visit。nullable等のrule通過も数えるため、単一arrayの件数上限とは別の制約となる。anyやKeepでそのまま通した未知subtreeの全内容を検証したことにはならない。VMのwork/allocation上限は別に適用する。

## CSV row adapter

`csvRow(schema,&headers,&row,nullToken)`はrootがobjectのschemaに、1行分のヘッダーと値を対応付けます。CSV streamそのものや複数行を保持せず、既存の`std.csvStream.next`が返す`List<String>`を1行ずつ渡せます。

headerは一意で各128 scalar以下、rowはheaderと同じ列数です。列は最大65,536、row全体は最大1,048,576 scalarです。schema field名はMap indexで引くため、headerごとに全schema fieldを走査しません。既定値と必須field、Unknown::Reject/Keep/Dropは通常の`validate`と同じ規則です。

TextとAnyは常にCSV cellをそのまま文字列として扱います。Boolean / Integer / Number / Array / Object fieldはcell全体をJSON値としてparseし、既存schemaで型と範囲を検査します。配列や入れ子objectは1つのCSV cellにJSON表記で格納します。型を推測したり、Textを数値へ暗黙変換したりしません。

`nullToken`はnullable nodeにだけ適用する完全一致文字列です。`None`は暗黙のNULL tokenなし、`Some("NULL")`ならそのcellだけNULLになります。空文字は既定では空のTextです。NULL tokenが空文字ならnullable列で空cellをNULLとして扱います。sentinelの長さは128 scalar以下です。必須fieldの列自体がheaderにない場合はMissingField、headerはあるがrow cellが足りない場合はRowWidthです。

重複header・過長header・row上限・cell JSON parse失敗は、入力文字列を含まないValidationErrorで返します。未知field名を含む診断は行いません。CSV parserのchunk上限・queue・EOF・cancelは既存readerが管理し、このAPIは受け取ったrowの変換だけを担当します。

利用例は[typed-csv-row.rw](../examples/typed-csv-row.rw)。sourceを保存済み。実行検証は省略。

## CSV stream readerとの接続

`std.csvStream.nextTyped(&mut reader,&headers,schema,nullToken)`は次のqueued rowを`schema.csvRow`へ渡します。EOFならNone、成功したJSONならreaderを一度だけadvanceしてSome(normalized)を返します。`TypedRowError::Stream`はreader側、`TypedRowError::Validation`はCSV row/schema側の失敗です。

Validation failureではreaderをadvanceしないので、schema/headerを直して同じrowを再試行できます。rowを捨てる場合は通常の`next`でraw rowを消費します。queue上限に達したfeed chunkのretry、chunk size、EOF、cancelは引き続きcallerが制御します。nextTypedはbatchを蓄積せず、1 callに1 rowだけ返します。

[CSV stream example](../examples/csv-stream/README.md)は型の異なるschemaで失敗を確認した後、同じrowを正しいschemaで変換します。execution verificationは省略。

## 型付き変換

mapped(schema,convert)はDecoder<T,E>を作り、decodeは検証・正規化の成功後にだけpureな明示mapperを呼ぶ。失敗はDecodeError<E>のValidation / Conversion段階で区別し、validationかconversionの対応する原因を保持する。

mapperにはSend+Share captureとShareのT/Eを要求する。変換Errorの内容は利用者のcallbackが決めるため、そのcallbackが機密値を返さない方針は利用者が維持する。native資源を勝手にJSON化することや、private fieldのreflectionは行わない。

Schemaはdataのみでsnapshotできる。Decoderはfunctionを含むため、既存freezeのfunction制約に従う。callback panicやfatal budget failureを通常のschema errorへ変換しない。

## 費用と検証状態

schema合成ではchild node表をcopyし、上限を事前に検査する。同じchildを自動的に共有・hash-consする実装ではない。objectの既知名はMapでlookupし、入力keyごとに全schema fieldを走査する方式を避ける。正規化するobject/arrayは新しいcontainerを構築する。

compile・実行・API生成・性能/GC測定は省略。libraries/std/tests.rwへ5件の受入用sourceを追加した。missing/NULL/default、入れ子path、unknown policy、値を含めない診断、mapperの失敗段階、depth/node上限とsnapshotを扱う。利用例は[typed-schema.rw](../examples/typed-schema.rw)。expected出力は未実行の期待値。公開CI/CD、main統合、Release、版更新は行わない。
