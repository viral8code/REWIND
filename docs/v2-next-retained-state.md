# 構想の具体化：checkpoint外の変数領域

状態：採用判断前。未実装です。ユーザー提案の「commitに影響しないハイパーグローバルな変数宣言」を、巻き戻されないVM内stateとして考えます。構文・型名・effect名は仮です。

## 二つの独立した軸

変数がどこから見えるかというscopeと、checkpointで保存・復元するかという寿命は別の性質です。初期案ではmodule/privateの公開境界を維持しながら、checkpoint外へ保持する宣言を追加します。他moduleのprivate値へ無条件にアクセスできるglobal namespaceにはしません。

この領域はOSやfileへ永続化するものではなく、一つのruntime実行中に保持するものです。通常終了・runtime破棄で解放します。process再起動後の保存は別の機能です。

## 利用例

以下は**仮の構文を使う擬似コード**です。現在のREWINDでは使えません。

~~~text
hyper var attempts = 0;
var score = 10;
commit retry;

attempts += 1;
score = 99;
revert retry;

// scoreは10へ戻る。
// attemptsは1のまま。
~~~

同じcheckpointへresumeしても試行回数を残す、Undo対象のmodelと履歴管理を分ける、純粋な探索の統計を残す、といった使い方ができます。通常の変数まで暗黙にこの領域へ移しません。

## HS-01：巻き戻されないbindingと値

commitにこの領域の版を含めず、通常revert/resumeで上書きしません。bindingだけ除外して、その参照先の可変objectは通常heapへ残す設計では、値の一部だけ戻ってしまいます。

scalarから始める案と、独立したowner graphを持つcollectionまで許す案を比較します。declarationのidentityをmodule/定義に対応させ、resumeで同じ宣言を通過したときにinitializerが再実行されて値を消さない規則を決めます。初期案はmodule-levelの宣言から考えます。

## HS-02：beginとの関係

現行beginはVMの初期化checkpointです。この領域を新しく設ける場合、次の二案は意味が異なります。

- 通常revertでは保持し、revert/resume beginではこの領域も初期化する。
- beginでも保持し、明示resetまたはruntime破棄でのみ初期化する。

後者は「ユーザーの巻き戻されないstate」として一貫し、beginを使う再試行の回数も残せます。ただし現行の「何もかもリセット」という期待へ例外を追加するため、採用時に明記する必要があります。現在のbegin仕様は変更していません。

独立したreset操作を設ける場合は、保持中snapshot・他task・resourceの扱いも決めます。resetを通常のrevertと同義にしません。

## HS-03：通常heapとの境界

checkpoint後に作ったmutable値への参照を保持し、revertで元heapだけ失われることを防ぐ必要があります。

候補は、scalar/immutable値のcopy、独立Snapshot<T>の保持、専用領域への明示owner移動です。通常heapへのmutable aliasや借用を長期保持する入口は避けます。Snapshot<T>は旧pageをrootとして保持できる一方、閉じたphysical resourceを復活させません。

通常領域へ値を戻すときも、copy/immutable共有/owner移動を明示します。checkpoint対象と対象外の同じ可変objectを同時に更新できる共有にはしません。

## HS-04：履歴registryと永続データ構造

[動的checkpoint・snapshot構想](v2-next-checkpoint-api.md)のversion一覧をこの領域へ置けると、全VMをrevertしても履歴を管理するListが消えません。

ただし古いCheckpointIdを保持することと、そのcheckpointが復元可能であることは別です。beginやdropで無効になったhandleは無効のままです。独立data snapshotを保持する場合は、自身のowner/rootによって値を読むという設計にします。

結果として「通常modelは戻す」「版のcatalogは保持する」「過去のdataはsnapshotで読む」をlibraryとして組み合わせられます。

## HS-05：branch・task・effect

branchの中でこの領域へ書けば、親へ戻っても残る変更になります。branchが完全に何も残さない試行だという期待とは区別し、読取り/書込みに明示effectや許可を持たせる案です。

main-taskだけが書く初期案と、taskごとの領域、同期付きruntime共通領域を比較します。共有mutable globalを追加しただけで並列安全にはしません。scheduler選択とrecord/replayに更新順を残します。

任意のdebug watchやpreviewから書込みません。external領域で触れるかどうかも通常のeffectと別に判断します。

## HS-06：GC・容量・情報保護

この領域もuserが保持するmemoryなので、GC rootとmemory予算に含めます。revertで回収されないからといって無制限に使える領域にはしません。

snapshot registryのcycle、弱いname索引、明示release/reset、終了時解放を調べます。profileでは通常state・checkpoint・checkpoint外stateの保持理由を分けて表示します。Secretと既存redactionの契約を維持します。

## 名前と採用判断

hyper/globalという名前は候補ですが、公開scopeと巻き戻しの性質を混同しやすいので、retained、untracked等も比較します。untrackedという名前でもmemory会計やrecordをしない意味にはしません。

この案はVM内の第三の状態領域です。通常のsnapshot対象state、ユーザーが保持するcheckpoint外state、外部観測・公開済み作用のledgerを区別します。実装に進むなら、scalarと独立snapshotを保持する小さな例から検証します。
