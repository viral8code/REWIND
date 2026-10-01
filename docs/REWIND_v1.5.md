# REWIND v1.5.0 草案

v1.4.0 を基準に、VM 内の巻き戻しと即時外部作用の境界を作る。これは未実装の設計案。[全体計画](ROADMAP_v2.md) と [詳細設計](v2-design.md) の外部領域・資源・scheduler の章を共通契約とする。HTTP と DB は v1.6 / v1.7 の対象。

## 範囲

この版では構文、effect、操作記録、再実行防止、replay、失敗区分、資源 token の基本契約を整える。単純な時刻観測とテスト adapter を使って外部境界を確認する。connection を実際に開く API は次の版で追加し、未実装の HTTP / DB を提供済みとして記載しない。

## 明示領域の提案

```text
var observed = 0;
commit before;
external {
    observed = /* 外部 API の Result を確認して取り出す */;
}
revert before;
external {
    observed = /* 同じ論理操作なら記録済みの結果を使用 */;
}
external fresh {
    observed = /* 意図した新規操作 */;
}
```

この例は構造の説明であり、現在実行できるコードではない。外部 API の結果は Result と通常の不変値へ変換する。

| 項目 | 契約案 |
| --- | --- |
| `external { ... }` | 同じ操作位置・同じ要求の結果を再利用し、新しい位置でのみ host を呼ぶ |
| `external fresh { ... }` | 配信済み位置から新しい操作を始める。事前読込みした将来の replay 記録は読み飛ばさない |
| effect | `external` と adapter ごとの effect を明示する。region が CLI / manifest の権限を増やさない |
| 通常の値 | region 内での変数・heap 変更も VM state として巻き戻す |
| region の終了 | publish ではない。既存の未公開 output / file / scene を自動確定しない |
| checkpoint 操作 | commit / revert / resume / drop / branch / publish を region 内で拒否する |
| 要求の不一致 | 自動再送せず `ExternalRequestMismatch` とする |
| 結果不明 | `ExternalOutcomeUnknown` として保持し、revert 後も再試行しない |

region を helper / closure / 関数値で迂回できないよう、静的検査に加えて VM / Runtime で検査する。外部呼出しに必要な region を推論し、effect と分けて扱う。内部で region を完結させる helper は通常の VM 領域から呼べるようにする。

return / `?` / break / continue が region を抜ける際には、scope cleanup と region context を必ず解除する必要がある。初版で対応できない形は静的に拒否し、制限を明記する。helper 内の return を一律禁止する設計にしない。非同期 region は task ごとの context が揃うまで拒否し、v1.6 の host I/O とともに拡張する。

## 操作記録

操作の識別と結果は checkpoint 外の ledger に置く。VM が持つ操作の論理位置は checkpoint の対象にする。begin / resume でも ledger を消さない。

記録には論理操作 ID、adapter / schema 版、要求 fingerprint、資源の論理 ID、結果・失敗区分、完了状態を含める。要求の public な入力と秘密入力の照合を区別し、credential を記録や診断へ出さない。task / branch への拡張時は別経路で ID が衝突しない識別を設計する。

1. 引数、効果、region、work、最大結果容量を検査する。
2. 記録枠と buffer を送信前に予約する。失敗したら host を呼ばない。
3. host を一度だけ呼ぶ。in-flight / 完了を同じ操作 ID に結び付ける。
4. 結果・失敗を記録してから VM に返す。
5. host 完了後のローカル記録失敗は、未適用と扱わず結果不明として止める。

本文や数値を含む大きい結果は immutable block として共有し、checkpoint ごとの深い copy や JSON 往復を避ける。記録にも累積 memory / storage 上限を適用する。prefix の回収・spill の詳細は 1.9 の保持監査までに整え、無制限に蓄積しない。

## 外部の失敗と資源

失敗を、送信前の未適用、確認済みの完了、部分適用、結果不明に分類する。timeout / cancel は「相手が未適用」の証明にしない。自動 retry は禁止し、fresh による新規操作も外部での二重適用を防ぐ保証にはしない。

native resource は Runtime 所有の registry と型付き token で管理する。token の世代・種別・closed を検査し、snapshot に残る古い token から資源を復活させない。close / scope cleanup / Runtime 終了を資源 lifetime の境界とし、GC へ DB commit 等を委ねない。

DB transaction の commit / rollback は後続 adapter の外部操作。VM の commit / revert や region の終了とは独立させる。

## replay と秘密

replay は記録済み結果を返し、外部の接続・送信・書込みを行わない。記録が尽きたり要求が変わったりしたら停止する。live host への fallback は禁止する。

秘密値・認証 header・機密 parameter を通常の記録・出力・診断に含めない。結果の秘匿により後続の計算を再現できない場合は、その trace の完全 replay を拒否するか別入力を要求する。値を省略したまま完全再現と呼ばない。

in-process の再送防止と、crash / 再起動後の exactly-once を区別する。この版で後者は保証しない。

## 変更対象

parser / AST、checker / effects、ownership / capture、VM / Runtime、artifact / cache、observation schema、REPL / formatter / LSP、source std / SDK metadata / API snapshot を同じ版で変更する。共通の変更対象は [詳細設計](v2-design.md) の最初の章を参照する。

既存の publish と入力 journal の意味は維持する。新規の外部 region を導入したことだけで、従来の file / GUI API をすべて即時作用へ変更しない。

## 受入条件

| ケース | 確認する結果 |
| --- | --- |
| checkpoint に戻り同一操作を通る | host 呼出しは一回、同じ結果を返す |
| 同じ位置で別要求を使う | host を呼ばず不一致エラー |
| fresh で意図的に再実行 | 新しい記録となり、既存記録を上書きしない |
| future 記録を事前読込みした replay | fresh で未配信分を飛ばさない |
| 記録予約が容量不足 | 外部作用を実行しない |
| 適用後に記録できない / 応答が不明 | 結果不明を保持し、revert 後も再送しない |
| helper / closure 内の checkpoint | 静的検査または runtime guard で境界を守る |
| region の正常・エラー終了 | context を誤って残さず、cleanup の cause を保持する |
| begin / resume / branch | VM state と操作位置が戻っても外部事実を保持する |
| 閉じた token を保持する snapshot | 資源を復活させず失効を診断する |
| 秘密値を含む操作と trace | 記録・診断への露出を防ぎ、replay 条件を説明できる |
| source-free / 両 OS SDK | 構文・API・effect・記録契約がソース実行と一致する |

失敗ケースを fixture だけで終えず、host 呼出し回数・記録状態・memory を計測する。公開は回帰テスト・標準契約・SDK 検証を経て行い、公開成功後に main へ統合する。
