# 暗号library・鍵・認証されたdata

一般的な情報flowや署名の構想を、鍵の寿命とdata形式へ広げる。新しい暗号方式を独自に発明する目的ではなく、検証されたproviderを使う型付きinterfaceの候補を考える。

## VCRYPTO-01：用途を限定したkey型

暗号化、署名、MAC、鍵導出等のkeyを用途とalgorithmの型で分ける。異なる用途の生bytesを同じAPIへ渡す誤りを減らしたい。

keyの所有者、provider、export可否を持たせる。表示・serializationの一般deriveで秘密鍵を取り出す設計にはしない。

## VCRYPTO-02：nonceの発行owner

AEAD等が要求するnonceの一意性を、keyと発行ownerのcontractにする。通常modelのrevertで発行済みnonceを再利用しない。

processを跨ぐ一意性が必要なら専用永続storeやproviderへ結び付ける。hyper counterだけで再起動後の一意性が保証されるとはしない。

## VCRYPTO-03：用途ごとのrandom source

simulationの再現用乱数、試験fixture、暗号用entropyを別能力にする。key生成APIは暗号用途のsourceだけを受け取る。

記録再生用sourceを選んだtestを実運用のentropyと混同しない。providerの失敗は型付きerrorとして返し、既定seedへfallbackしない。

## VCRYPTO-04：認証後だけ読める復号結果

復号済みbytesを認証確認前に通常dataとして公開しない。Authenticated<T>等の返値を経てschemaへ変換する。

大きなstreamではchunk認証と全体の完全性を分ける。まだ末尾を確認していない値に、完全な文書というcontractを付けない。

## VCRYPTO-05：署名する表現の固定

recordを署名する際のcanonical representation、schema版、domain tagを明示する。

field順、Unicode表現、floatの特殊値等の規則を共有し、同じ見た目と同じ署名対象を区別する。任意の表示用formatterを署名形式へ流用しない。

## VCRYPTO-06：認証されたsnapshot pack

root、page、schema、format版を署名またはMACの対象にする。部分読込みでも、そのpageが期待rootに属するかを確認したい。

内容のhashが一致することと、信用する主体がその内容を認証したことを分ける。権限やnative handleを復元できる形式にはしない。

## VCRYPTO-07：key rotationのdata plan

どのdataがどのkey版で保護されているかを一覧にし、再暗号化の進捗・失敗・未処理を記録する。

keyを無効にした後も、古いsnapshotが存在することを説明する。data modelのrevertでprovider側のkey失効を取り消せるとは扱わない。

## VCRYPTO-08：constant-time contractのbackend

機密値に依存した分岐やmemory accessを制限する専用primitiveを、検証されたbackendへ渡す。

通常のVM bytecodeへ注釈を付けただけでside channel耐性を保証しない。対応backendと検査範囲をcontractに含めたい。

## VCRYPTO-09：秘密値のallocation policy

機密bufferのcopy、pin、native transfer、解放時の処理を明示する。debugger・trace・snapshotの既定exportでは値を出さない。

管理runtime内で生じるcopyや古いsnapshotがある場合、即時完全消去を無条件に約束しない。必要な保持範囲を確認できる体験を考える。

## VCRYPTO-10：鍵導出の費用指定

password由来のkeyに、memory・仕事量・parallelism等のpolicyを持たせる。環境に合わせたparameterを測定し、保存形式へ記録する。

攻撃耐性の設計値と、service側の同時実行budgetを両方扱う。入力dataで無制限のallocationを要求できないinterfaceにしたい。

## VCRYPTO-11：証明書の判定結果

chain、用途、hostname、時刻、失効情報等を型付きの検証結果へ持たせる。判定に使ったpolicyと情報の鮮度も残す。

parseできた証明書と、利用目的を満たす証明書を区別する。物理時刻や外部失効情報を、revert可能なsimulation clockへ黙って置き換えない。

## VCRYPTO-12：providerの能力交渉

OS store、hardware token、software library等を共通signatureへ接続する。対応algorithm、export、非同期操作、寿命制約を確認して選ぶ。

provider間で同じ保証が得られない場合は能力の差として返す。高速化・互換性のために、知らないうちに弱い方式へ切り替えない構成を考える。
