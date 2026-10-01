# REWIND v1.5.0

VM の巻き戻しと即時外部操作の境界を追加する。外部操作の結果を記録し、同じ位置へ戻っても二重実行しない。HTTP / DB は後続版の対象であり、この版には含まない。

## 実装

- `external { ... }` と `external fresh { ... }`。native 操作の region 要求を検査し、Runtime にも境界 guard を置く。
- 操作 fingerprint と結果は checkpoint 外、論理 cursor は checkpoint 内に保持。同一位置・同一要求は結果を再利用し、別要求は `ExternalRequestMismatch`。
- fresh は配信済み high-water に cursor を進める。replay で事前読込みした未配信結果を読み飛ばさない。
- 結果の記録枠を host 呼出し前に予約する。host 終了後に記録できなかった操作は `ExternalOutcomeUnknown` として残し、自動 retry しない。
- Rust adapter 用の bounded resource registry と token。close / registry drop で所有資源を解放し、古い token と別 registry の token を拒否する。REWIND の接続型 API は後続版で追加する。
- `std.external.millis()->Result<Int,StdError>` は UTC milliseconds の外部観測。`external,clock` の明示許可が必要。
- observation tape に外部 schema を追加。replay は host を呼ばず、記録不足・要求の不一致を拒否する。
- 登録された秘密を含む要求を拒否し、秘密を含む結果の trace export は `SecretObservationUnrecordable`。値を欠落させた完全 replay を保証しない。
- 両 OS の SDK に module / API 文書と [外部操作サンプル](../examples/external/README.md) を同梱する。

## 利用

```sh
rewind run main.rw --allow-effects external,clock
rewind compile main.rw --allow-effects external,clock
rewind replay trace.json --root . --allow-effects external,clock
```

```rewind
import std.external as host;
var stamp=0;
commit before;
external {
    match host.millis() {
        Ok(n)=>{stamp=n;},
        Err(e)=>{Out.println(e.code);}
    }
}
Out.println(stamp);
publish;
revert before;
external {
    match host.millis() {
        Ok(n)=>{stamp=n;},
        Err(e)=>{Out.println(e.code);}
    }
}
Out.println(stamp);
publish;
```

二行に同じ値が出る。新しく時刻を観測する場合は `external fresh` を使う。region 内の通常の変数は VM state として巻き戻り、外部で確認された事実は残る。region の終了は publish ではない。

## 初版の保証範囲

main task の active branch 外で逐次実行する。領域内の commit / revert / resume / drop / branch / publish と task switching は拒否する。領域から抜ける return / `?` / break / continue と defer は静的に拒否し、helper 内の通常の return は許す。helper / closure / 関数値の禁止操作にも Runtime guard を適用する。

結果は操作ごと最大 1 MiB、要求も最大 1 MiB、記録は最大 100 万件。累積 history budget を適用し、trace の外部記録は 16 MiB まで。現時点で自動 prefix 回収・native connection API・非同期 region・HTTP / DB・crash recovery は提供しない。Rust embedding では host callback の入出力と資源所有権を caller も管理する。

in-process の再送防止とプロセス再起動後の exactly-once は別。結果不明の操作を外部で未適用と解釈しない。

## 検証

同一操作・別要求・fresh / replay high-water、記録予約失敗、適用後の記録失敗、host failure、資源 close / token 失効、秘密結果の export、helper の境界、ソース実行・record / replay・source-free を回帰テストで確認する。既存 publish / begin / GUI / ownership と標準契約も継続検証する。

次は [v1.6 草案](REWIND_v1.6.md) と [詳細設計](v2-design.md)。各版の両 OS 検証が成功してから main に統合し、Release を公開する。
