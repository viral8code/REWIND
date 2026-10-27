# Sketch：実行中のmoduleを版として更新する

## 使いたい体験

長く動くGUIや計算workspaceでcodeを更新し、現在のmodelを移行して、新しい操作から新しい実装を使う。旧taskや外部資源の状態も見ながら更新したい。

生きたstackやnative handleを新codeの形へ強制変換する仕様ではない。更新できるdataと、終了・再構築が必要な資源を分ける。

## 組み合わせる構想

- 版付きmodule identityとlink plan。
- schema migrationとdata snapshot。
- safe barrier、task supervision、async resultの入力版。
- effect能力とmoduleのpublic signature。
- version-aware stack frameとstructured Diagnostic。
- snapshot packと明示的な旧版保持。

## 擬似コード

~~~text
hyper var installed = ModuleVersionCatalog();
hyper var receipts = ReceiptRegistry();

let candidate = compileModule(source, expectedSignature);
let compatibility = compareContracts(activeModule, candidate);
let migration = candidate.planMigration(snapshot.capture(model));

show(compatibility, migration.preview);

on AdoptUpdate {
    let barrier = taskGroup.requestSafeBarrier();
    let readiness = barrier.inspect();

    readiness.requireCompatibleModelAccess()?;
    let migrated = migration.revalidateAndApply(model)?;

    let previous = InstalledDataVersion(activeModule, snapshot.capture(model));
    installed.pin(previous);

    model = migrated;
    activeModule = candidate;
    dispatch.useForNewCalls(candidate);
}
~~~

すべて仮のAPI。更新の採用と、VM全体を旧codeへresumeする操作は別のもの。

## 生きたframe

旧codeを実行中のframeは、そのcode版で完了させるか、定義したsafe pointで終了して再起動する。signatureが似ているだけでprogram counterを新codeへ移さない。

旧frameが新modelへ直接書き込まないよう、旧版の結果を更新案として受け取る。採用時に現在版への対応を検査する。

## 外部資源

DB接続やnative handleが互換なら、ownerの移送contractに従って引き継ぐ。再構築が必要なら、受付停止、drain、close、再接続のplanを作る。

migrationを取り消しても、既に実行した外部操作はreceiptに残る。codeのrollbackは物理操作のUndoという表示にしない。

## 新旧moduleの型

public signatureが互換でも、旧版のopaque型と新版の型を同一と扱えるかは別に確認する。明示変換または共有した安定schemaを通す。

callbackやtrait objectが旧実装を保持する場合、そのownerの寿命まで旧moduleを残す。新しいcallのdispatchだけを替え、旧vtableのstorageを先に解放しない。

## 更新後に旧版へ戻す場合

保存したdata版に適用可能な逆migrationがある場合と、変換で情報を失った場合を分ける。旧入力snapshotを持っていることも、現在dataを完全に戻せる保証とは別。

old/new双方のschemaに対応するdata rootを選び、必要なtaskを再構築する。外部結果と照合して、再送が必要かではなく、どの処理が未実行かを判断する。

## もう少し広げるなら

更新の影響graph、特定viewだけの差替え、runの並行比較、旧版の保持費用、migration errorの最小再現をつなぐ。停止を減らしつつ、何を引き継いだか説明できる体験を考える。
