# parserをlibraryとして組み立てる

探索するparserのsketchを、grammar、error recovery、編集時の再parseへ広げる。generatorだけでなく、通常のREWIND codeから合成する方式も考える。

## VPARSE-01：出力型を持つcombinator

Parser<Input, T>を合成してrecordやvariantを返す。sequence、choice、repeat等の結果型が型検査で分かる。

消費せず成功するparserの無制限repeatは診断する。入力ownerとcursorの借用期間もcompositionへ引き継ぎたい。

## VPARSE-02：期待集合と失敗位置

失敗が期待したtoken集合、最も進んだ位置、選択したbranchを持つ。無関係な候補の大量のerrorをそのまま利用者へ出さない。

候補を戻しても最良の説明を残すpolicyを選ぶ。致命的なbudget終了は通常のgrammar不一致と別に返す。

## VPARSE-03：commitしたgrammar choice

曖昧なprefixを過ぎたら、この構文として続きを読むというcutを明示する。不要なbacktrackを減らし、error位置を保ちたい。

ここでのcutはparserの候補選択であり、言語のcommit labelとは別の操作。選択が可読性や結果へ与える影響をgrammar viewで示す。

## VPARSE-04：左再帰の処理policy

左再帰を検出して拒否、変換、専用の固定点評価等を選ぶ。式の文法を自然に書ける方式を比較したい。

memoizationと停止条件をplanへ持たせる。左再帰があるだけで全grammarが安全に評価できると仮定しない。

## VPARSE-05：operator tableから式を読む

precedence、associativity、prefix/postfix等をdataで宣言する。構文のtreeと適用規則を表示できる。

同順位の衝突やnon-associativeな連鎖は診断する。operator tableの版をparse cacheに含める。

## VPARSE-06：曖昧さを保つparse forest

複数の構文候補を共有forestへ残し、後の型検査やdomain ruleで選ぶ。全treeを独立copyせず扱いたい。

曖昧さの量にbudgetを付ける。候補を絞った理由と、未探索の候補を区別して表示する。

## VPARSE-07：原文を保つ構文tree

空白、comment、不完全tokenも含むlossless treeを返す。formatter、refactoring、editorが同じ入力表現を使う。

意味用ASTへ変換する場合もoriginを残す。commentを勝手に別nodeへ付け替えた編集を黙って確定しない。

## VPARSE-08：変更範囲からの再parse

text editと旧treeから、影響する領域だけを再parseする。再利用nodeはinput版とgrammar版へ対応する。

広い範囲に影響するlexical mode等は依存として追う。局所変更だから必ず局所parseで十分とは扱わない。

## VPARSE-09：error recoveryの候補

token挿入、削除、同期点へ進む等をcost付き候補にする。editorでは回復済みtreeとerrorを同時に返せる。

回復したtreeを、原文が正しくparseできた結果と区別する。compile時に許す回復範囲は別profileで選ぶ。

## VPARSE-10：stream入力と追加待ち

文法不一致と、まだ入力が足りない状態を分ける。chunk末尾で曖昧なprefixを保持して続きを待ちたい。

checkpoint可能なのは記録済み入力とcursorのdata。physical sourceを消費した事実は別adapterが扱う。

## VPARSE-11：grammarからの入力生成

grammarを使って小さなvalid inputや意図したinvalid inputを作る。parser、formatter、diagnosticの検査へつなぐ。

生成する深さとsizeを指定し、同じseedで再現する。grammarだけでは表せない意味制約も別のvalidatorとして使う。

## VPARSE-12：parse費用の閲覧

rule別の呼出し、backtrack、memo hit、最大深さを表示する。長い入力で急に遅くなる理由をtree上で調べたい。

profile取得を選択し、通常parseへ高い追跡費用を強制しない。費用の改善で結果・error・位置対応が変わらないか比較する。
