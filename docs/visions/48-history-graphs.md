# data履歴のgraph・branch・変更の運搬

checkpoint名と複数版の値の構想を、利用者が操作するdata履歴graphへ広げる。Gitに似た利用体験を考えるが、実行stackのcheckpointとdata versionを同じものにはしない。

## VHISTORY-01：型付きのHistory<T>

rootとparent関係、schema、annotationを持つ履歴を普通のlibrary値として使う。modelごとに独立した履歴を作りたい。

実行地点を含まないdata versionとして扱う。GUI callbackの終了後でも、選んだdataを履歴から読み出す用途に使える。

## VHISTORY-02：branchごとのHEAD

branch名が現在選んだimmutable version IDを指す。新しい版を作る操作と、HEADを移す操作を分ける。

branch名は変更できても、既に取得したversion handleは別の版へ変わらない。名前の再利用による誤参照を避ける。

## VHISTORY-03：履歴のnamespace

同じbeginやdraft等の名前を、異なるmodel、module、sessionで独立に使う。動的lookupはnamespaceのownerを受け取る。

名前解決の範囲を表示し、偶然の大域labelへ届かない構成を考える。実行checkpointの既存名前規則は別に照合する。

## VHISTORY-04：選択した変更のcherry-pick

ある版のChangeSetを別branchへ適用する。前提root、Entity対応、不変条件を検査して、新しい版を作る。

単に旧値をcopyする操作と、変更の意図を移す操作を分ける。適用できない場合は衝突と候補を返す。

## VHISTORY-05：変更列のrebase preview

共通baseから作った変更列を新しいbaseへ移す案を作る。各stepの適用結果、衝突、再検証を並べる。

採用前は元のbranchを変更しない。計算した外部operationのreceiptをrebaseで未実行へ戻す意味にはしない。

## VHISTORY-06：mergeの法則を持つstrategy

field、collection、domain型ごとにmerge policyを選ぶ。commutative、associative、idempotent等の性質を宣言・検査したい。

性質を持たないpolicyではparent順や利用者の選択を記録する。すべてのdataに一つの自動mergeを強制しない。

## VHISTORY-07：履歴の範囲を切るview

tag間、特定Entity、特定field等に絞ったhistory viewを作る。長い履歴の必要な部分だけを読む。

viewを保持することが元のrootをどこまで保持するか表示する。選択dataだけをdetachするexportも考える。

## VHISTORY-08：履歴annotationの独立版

理由、comment、評価等をdata内容とは別のannotation streamにする。説明の追加だけで大きなmodelの版を作り直さない。

annotationの著者や対象versionを保持し、元の計算結果を書き換えない。annotationの削除とdata rootの削除を分ける。

## VHISTORY-09：protectedな履歴root

利用者が保存したいrootへpinや保護policyを付ける。GCやcache掃除では消えず、明示的に解除してから整理する。

保護rootが多いmemory費用は利用者の保持として表示する。rootを勝手に捨てて性能上限を守ったことにはしない。

## VHISTORY-10：歴史上のproperty query

値が変化した版、条件を満たした区間、最初に不変条件が破れた版をqueryする。

queryは選んだhistory graphと純粋predicateに対して行う。物理世界の過去状態まで完全に保持していると仮定しない。

## VHISTORY-11：graphの圧縮と要約版

利用者が中間版を不要と判断した時、dataの現在rootと説明要約を保持して履歴を縮める。

別branchや独立snapshotが保持する版は影響を確認する。squash後に失う個別変更や再現情報をpreviewしたい。

## VHISTORY-12：履歴packの共有

選択したrootと必要な共有pageをpackにして別runtimeへ渡す。共通baseがあればdeltaだけを送る案も作る。

data version ID、runtime handle、実行checkpoint IDを分ける。import時は新しいownerとidentity検査を作り、外部権限を自動移送しない。
