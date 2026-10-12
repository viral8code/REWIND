# 宣言的JSON schemaと型付きdecoder

更新日：2026-10-12。対象：codex/develop。**source実装済み・実行検証省略**。

[DATA-01](v2-next-data.md)のJSON検証と明示mapperをstd.schemaへ実装した。既存std.configの入力優先順位とflatな設定検証は維持する。CSV/DB/form向け共通adapter、Decimal・日時向けrule、reflection/deriveは後続の範囲。

## Schemaの構築

any、boolean、number、integer(min,max)、text(min,max)、nullable、array、field、objectを提供する。numberはJsonのInt/Floatをそのまま受け、暗黙にFloatへ変換しない。textの長さはUnicode scalar。arrayは0〜65,536件の範囲を宣言する。

fieldは名前、子Schema、required、Option<Json>のfallbackを受ける。名前は1〜128 scalar。requiredとdefaultの同時指定はRequiredDefaultで拒否する。defaultはfield構築時に子Schemaで検証・正規化して保持し、不正ならInvalidDefault。NULLは欠損とは別の値であり、nullableを指定した場合だけ通常の型検証を省略して通る。anyはNULLを含む任意Jsonを受ける。

objectのUnknown policyはReject / Keep / Drop。fieldは宣言順で最大128件、一意の名前。Schema全体は1,024 node、深さ32まで。内部は有限なnode表と整数の子参照で表し、schemaの合成時に参照を移し替える。private constructorにより循環参照を作る入口は提供しない。

## 検証と結果

validateはdefault挿入とunknown policyを反映したJsonを返す。入力は変更しない。必須項目が欠損すればMissingField、明示NULLはdefaultで置き換えず、schemaに従って検査する。

エラーは最初の一件をValidationError(code,path,expected,actual)として返す。pathはField(String) / Index(Int)のFrozen列。実値、未知fieldの入力名、JSONの抜粋をerrorへ自動追加しない。expectedは型・制約の説明、actualはkindまたはMissing。Schema構築時のSchemaError.fieldは利用者が宣言したfield名を扱う。

objectではunknown policyを先に検査し、宣言済みfieldは宣言順、配列はindex順で進む。Unknown::Rejectのerrorは対象objectのpathを返す。全errorの収集や大量の連鎖診断は行わない。

一回のvalidateは最大65,536 validation visit。nullable等のrule通過も数えるため、単一arrayの件数上限とは別の制約となる。anyやKeepでそのまま通した未知subtreeの全内容を検証したことにはならない。VMのwork/allocation上限は別に適用する。

## 型付き変換

mapped(schema,convert)はDecoder<T,E>を作り、decodeは検証・正規化の成功後にだけpureな明示mapperを呼ぶ。失敗はDecodeError<E>のValidation / Conversion段階で区別し、validationかconversionの対応する原因を保持する。

mapperにはSend+Share captureとShareのT/Eを要求する。変換Errorの内容は利用者のcallbackが決めるため、そのcallbackが機密値を返さない方針は利用者が維持する。native資源を勝手にJSON化することや、private fieldのreflectionは行わない。

Schemaはdataのみでsnapshotできる。Decoderはfunctionを含むため、既存freezeのfunction制約に従う。callback panicやfatal budget failureを通常のschema errorへ変換しない。

## 費用と検証状態

schema合成ではchild node表をcopyし、上限を事前に検査する。同じchildを自動的に共有・hash-consする実装ではない。objectの既知名はMapでlookupし、入力keyごとに全schema fieldを走査する方式を避ける。正規化するobject/arrayは新しいcontainerを構築する。

compile・実行・API生成・性能/GC測定は省略。受入用sourceと利用例は後続commitで追加する。公開CI/CD、main統合、Release、版更新は行わない。
