#!/usr/bin/env python3
"""Build one offline HTML volume from reviewed prose and the v2.0.0 API snapshot.

Dependency: python -m pip install -r docs/book/requirements.txt
Regenerate: python scripts/build-book.py
"""
import hashlib, html, json, re
from pathlib import Path
import mistune

ROOT=Path(__file__).resolve().parents[1]
BOOK=ROOT/"docs"/"book"
data=json.loads((BOOK/"api-snapshot.json").read_text())
notes=json.loads((BOOK/"modules.json").read_text())
contracts=json.loads((BOOK/"examples.json").read_text())
validation=json.loads((BOOK/"validation.json").read_text())
results={item["id"]:item for item in validation["examples"]}
if validation["passed"]!=validation["total"] or set(results)!=set(contracts):
    raise SystemExit("Run check-book-examples.py successfully before publishing the book")
if set(notes)!=set(data["modules"]):
    raise SystemExit("Module descriptions and SDK exports differ")
esc=html.escape
headings=[]
search=[]
seen_examples=set()
chapter=0
section=0
def plain(value):
    return html.unescape(re.sub(r"<[^>]+>"," ",value)).strip()
def codebox(code,label="REWIND",ident=None):
    attrs=f' id="example-{esc(ident)}"' if ident else ""
    action='<button type="button" class="copy" aria-label="コードをコピー">コピー</button>'
    download=f'<button type="button" class="download" data-filename="{esc(ident)}.rw">.rw保存</button>' if ident else ""
    return f'<figure class="codebox"{attrs}><figcaption>{esc(label)}<span>{action}{download}</span></figcaption><pre><code>{esc(code.rstrip())}</code></pre></figure>'
class Renderer(mistune.HTMLRenderer):
    def heading(self,text,level):
        global chapter,section
        if level==1:
            chapter+=1;section=0;ident=f"ch{chapter:02}"
        else:
            section+=1;ident=f"ch{chapter:02}-s{section:02}"
        headings.append((level,ident,plain(text)))
        return f'<h{level} id="{ident}" tabindex="-1">{text}<a class="anchor" href="#{ident}" aria-label="この節へのリンク">#</a></h{level}>\n'
    def block_code(self,code,info=None):
        pieces=(info or "").split()
        language=pieces[0] if pieces else "text"
        ident=pieces[1] if language=="rewind" and len(pieces)>1 else None
        if ident:
            seen_examples.add(ident)
            proof=results[ident]
            if hashlib.sha256(code.encode()).hexdigest()!=proof["source_sha256"]:
                raise ValueError(f"Example changed after verification: {ident}")
            label=f"REWIND・実行例 {ident}"
            body=codebox(code,label,ident)
            cfg=contracts[ident]
            instructions=[]
            if cfg.get("effects"):instructions.append(f"必要な許可: {cfg['effects']}")
            if cfg.get("args"):instructions.append("引数: -- "+" ".join(cfg["args"]))
            if cfg.get("http"):instructions.append("引数: -- URL（検証時はローカルHTTP server）")
            if cfg.get("fixture"):instructions.append("GUI検証は入力fixtureを使用。通常実行にはnative表示環境が必要。")
            if cfg.get("steps"):instructions.append(f"検証予算: steps={cfg['steps']}, native-work={cfg.get('native_work')}")
            if instructions:body+=f'<p class="example-note">{esc(" / ".join(instructions))}</p>'
            output=cfg["stdout"]
            label="標準出力（検証済み）" if output else "標準出力なし・正常終了を検証済み"
            body+=f'<details class="expected"><summary>{label}</summary><pre>{esc(output) if output else "空（assertを含む処理は正常終了）"}</pre></details>'
            return body+"\n"
        label={"syntax":"構文・説明用断片","sh":"CLIコマンド","toml":"設定例","powershell":"PowerShell","text":"表示例"}.get(language,language)
        return codebox(code,label)+"\n"
renderer=Renderer(escape=True)
markdown=mistune.create_markdown(renderer=renderer,plugins=["table"])
worked=[]
for item in json.loads((BOOK/"worked-examples.json").read_text()):
    source=(BOOK/item["source"]).read_text()
    worked.append(f"## 実習：{item['title']}\n\n{item['description']}\n\n~~~rewind {item['id']}\n{source.rstrip()}\n~~~\n")
text="\n\n".join(path.read_text() for path in sorted((BOOK/"chapters").glob("*.md")))
text=text.replace("{{worked-examples}}","\n\n".join(worked))
prose=markdown(text)
if len([h for h in headings if h[0]==1])!=32:raise ValueError("Expected 32 chapters")
if seen_examples!=set(contracts):raise ValueError("Rendered/verified example mismatch")
# Separate searchable sections; every result is an existing stable anchor.
pieces=re.split(r'(?=<h[12] id=")',prose)
for piece in pieces:
    match=re.search(r'<h[12] id="([^"]+)"[^>]*>(.*?)</h[12]>',piece,re.S)
    if match:search.append(dict(id=match[1],title=plain(match[2]).rstrip("#"),text=plain(piece)))

def schema_block(key,value):
    name=key.split(":",1)[1]
    kind=key.split(":",1)[0]
    if kind=="alias":
        return codebox(f"type {name} = {value['target']};","型alias")
    if kind=="type":
        generics=value.get("generics",[])
        names=[g[0] if isinstance(g,list) else g for g in generics]
        title=name+("<"+",".join(names)+">" if names else "")
        if value.get("kind")=="enum":
            lines=[]
            for variant,fields in value["variants"].items():
                payload="("+", ".join(field[1] for field in fields)+")" if fields else ""
                lines.append(f"    {variant}{payload}")
            return codebox(f"enum {title} {{\n"+",\n".join(lines)+"\n}","variantとpayload型")
        fields=value.get("fields",[])
        body="\n".join(f"    {field[0]}: {field[1]}" for field in fields)
        suffix="（公開fieldなし。専用APIで生成・操作）" if not fields else "（公開field型）"
        return codebox(f"{value.get('kind','type')} {title} {{\n{body}\n}}","型 "+suffix)
    return codebox(json.dumps(value,ensure_ascii=False,indent=2),"公開契約schema")

api=['<section class="api-intro"><h1 id="api" tabindex="-1">標準ライブラリ API編</h1><p>全86公開モジュール・625関数。各宣言には引数名、generic bound、戻り型、効果を含めています。共通型は末尾へまとめています。</p><div class="module-index">']
toc_api=[]
for name,module in data["modules"].items():
    api.append(f'<a href="#mod-{name}">std.{name}</a>')
api.append('</div></section>')
function_count=0
for name,module in data["modules"].items():
    source=(ROOT/"libraries"/"std"/f"{name}.rw").read_text()
    if hashlib.sha256(source.encode()).hexdigest()!=module["source_sha256"]:
        raise ValueError(f"Standard source changed; reimport SDK API: {name}")
    ident=f"mod-{name}";title=f"std.{name}"
    toc_api.append((ident,title))
    api.append(f'<section class="module" aria-labelledby="{ident}"><h2 id="{ident}" tabindex="-1">{title}<a class="anchor" href="#{ident}" aria-label="このmoduleへのリンク">#</a></h2><p class="module-description">{esc(notes[name])}</p>')
    api.append(codebox(f"import std.{name} as {name};","import"))
    search.append(dict(id=ident,title=title,text=notes[name]+" "+title))
    own={k:v for k,v in module["symbols"].items() if k.startswith(("type:","alias:","trait:","const:"))}
    if own:
        api.append('<h3>module固有の公開型</h3>')
        for key,value in own.items():
            api.append(schema_block(key,value))
    api.append('<p class="type-note"><a href="#common-types">共通型のfield・variant</a> / <a href="#source-'+name+'">このmoduleの実装を読む</a></p>')
    for fname,contract in module["functions"].items():
        function_count+=1;fid=f"fn-{name}-{fname}";signature=module["signatures"][fname]
        api.append(f'<article class="function" aria-labelledby="{fid}"><h3 id="{fid}" tabindex="-1">{title}.{fname}<a class="anchor" href="#{fid}" aria-label="この関数へのリンク">#</a></h3>')
        api.append(codebox(signature,"公開宣言"))
        effects=", ".join(contract["effects"]) or "なし（effects {}）"
        api.append('<dl class="contract"><dt>効果</dt><dd>'+esc(effects)+'</dd><dt>canonical戻り型</dt><dd><code>'+esc(contract["return"])+'</code></dd>')
        if contract.get("generics"):
            bounds=", ".join(f"{pair[0]}: {pair[1]}" if pair[1] else pair[0] for pair in contract["generics"])
            api.append('<dt>generic契約</dt><dd><code>'+esc(bounds)+'</code></dd>')
        if contract.get("cost"):api.append('<dt>費用契約・原文</dt><dd lang="en">'+esc(contract["cost"])+'</dd>')
        if contract.get("failure"):api.append('<dt>失敗・境界契約・原文</dt><dd lang="en">'+esc(contract["failure"])+'</dd>')
        api.append('</dl></article>')
        search.append(dict(id=fid,title=f"{title}.{fname}",text=signature+" "+json.dumps(contract,ensure_ascii=False)))
    api.append(f'<details class="implementation" id="source-{name}"><summary>std.{name}の実装を読む（v2.0.0）</summary>')
    api.append(codebox(source,"標準moduleの実装"))
    api.append('</details></section>')
if function_count!=625:raise ValueError(f"Expected 625 exports: {function_count}")
api.append('<section class="common-types"><h1 id="common-types" tabindex="-1">共通型付録</h1><p>SDKのAPI schemaが持つ共通型のpayload・公開fieldです。native資源は専用factoryを使います。以下のfield型は任意のphysical接続をconstructorで作れる保証ではありません。</p>')
for key,value in data["common_symbols"].items():
    ident="type-"+key.split(":",1)[1]
    api.append(f'<h2 id="{ident}" tabindex="-1">{esc(key.split(":",1)[1])}</h2>'+schema_block(key,value))
    search.append(dict(id=ident,title=key.split(":",1)[1],text=json.dumps(value,ensure_ascii=False)))
# SDK documentation includes intrinsic diagnostics omitted from exported API snapshots.
common_decls=set(next(iter(data["modules"].values()))["declarations"])
for module in data["modules"].values():common_decls.intersection_update(module["declarations"])
extra=[]
for declaration in sorted(common_decls):
    match=re.match(r"pub (?:struct|enum) (\w+)",declaration)
    if match and "type:"+match[1] not in data["common_symbols"]:
        extra.append(declaration)
api.append('<h2 id="intrinsic-types" tabindex="-1">診断・Task・propertyの組込み型</h2><p>次はSDK文書の組込み宣言です。enumのvariant名一覧はpayloadを省略しています。TaskErrorの詳しい診断はtaskError.describeのFailureを参照してください。CLIのframes/hints追加情報は診断JSONの契約です。</p>')
for declaration in extra:api.append(codebox(declaration,"SDK組込み型宣言"))
api.append('<h3 id="task-error-payload">TaskErrorとWaitTargetのpayload</h3><p>SDKのvariant名一覧で省略されているpayloadを、v2.0.0のsrc/v2/v05.rsと照合した補足です。</p>')
api.append(codebox('enum TaskError {\n    BudgetExceeded(BudgetKind),\n    Cancelled,\n    ChannelClosed,\n    Deadlock(WaitGraph),\n    Failed(Diagnostic),\n    TimedOut\n}\nenum WaitTarget {\n    Task(Int),\n    Channel(Int),\n    Group(Int)\n}', '型の構造・実装照合'))
search.append(dict(id='task-error-payload',title='TaskError / WaitTarget payload',text='enum TaskError {\n    BudgetExceeded(BudgetKind),\n    Cancelled,\n    ChannelClosed,\n    Deadlock(WaitGraph),\n    Failed(Diagnostic),\n    TimedOut\n}\nenum WaitTarget {\n    Task(Int),\n    Channel(Int),\n    Group(Int)\n}'))
api.append('</section>')
api.append('<section class="provenance"><h1 id="verification" tabindex="-1">出典と検証</h1><p>本文はREWIND 2.0.0の実装・言語リファレンス・各機能の契約を照合して編集しました。全60実行例について、source実行とsourceを削除したartifact実行の出力を照合しました。うち8例はartifactのcompact recordを、sourceとGUI入力fixtureに依存せずreplayしました。GUI実行例はfixture検証です。今回の実行例検証環境はLinux x86_64です。</p>')
api.append('<dl class="contract"><dt>対象compiler / language</dt><dd>2.0.0</dd><dt>対象実装commit</dt><dd><code>'+data["implementation_commit"]+'</code></dd><dt>参照SDK SHA-256</dt><dd><code>'+data["sdk_archive_sha256"]+'</code></dd></dl>')
api.append('<p>APIは公開Linux SDK内のstd-api.json、86組のmodule.api.json / module.mdから取得しています。標準moduleのsourceはSDKとrepositoryをbyte単位で一致確認し、HTML生成時にもSHA-256を照合しています。歴史的な草案を現在の未実装項目として再掲していません。</p><p>再生成用の本文、snapshot、例、期待出力、検証scriptはrepositoryのdocs/bookとscriptsにあります。PDFはこのHTMLの印刷用表示から生成できます。HTML/PDFの編集日: 2026-10-10。</p><p><a href="https://github.com/viral8code/REWIND/releases/tag/v2.0.0">対象Release</a> · <a href="https://github.com/viral8code/REWIND/tree/d0f3ee6684769d3f92dfb0e21c55deffc00dc181">対象実装の固定版</a></p></section>')
toc='<nav aria-label="本の目次"><ol class="chapters">'+''.join(f'<li><a href="#{ident}">{esc(title)}</a></li>' for level,ident,title in headings if level==1)+'</ol><details><summary>全章の節を表示</summary><ul class="sections">'+''.join(f'<li><a href="#{ident}">{esc(title)}</a></li>' for level,ident,title in headings if level==2)+'</ul></details><h2>APIを引く</h2><a href="#api">全モジュール一覧</a><details><summary>86モジュール</summary><ul>'+''.join(f'<li><a href="#{ident}">{title}</a></li>' for ident,title in toc_api)+'</ul></details><p><a href="#common-types">共通型付録</a><br><a href="#verification">出典と検証</a></p></nav>'
print_toc='<section class="print-toc"><h1 id="print-toc">目次</h1><ol>'+''.join(f'<li><a href="#{ident}">{esc(title)}</a></li>' for level,ident,title in headings if level==1)+'</ol><h2>標準ライブラリ</h2><div class="module-index">'+''.join(f'<a href="#{ident}">{title}</a>' for ident,title in toc_api)+'</div><p><a href="#common-types">共通型付録</a> · <a href="#verification">出典と検証</a></p></section>'
payload=json.dumps(search,ensure_ascii=False,separators=(",",":")).replace("<","\\u003c").replace(">","\\u003e").replace("&","\\u0026")
css=(BOOK/"style.css").read_text()
js=(BOOK/"reader.js").read_text()
document='<!doctype html><html lang="ja"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta name="description" content="REWIND 2.0.0の入門書・言語仕様・全標準ライブラリAPI。オフラインで読める32章と60の検証済み実行例。"><title>REWIND 2.0.0 入門とリファレンス</title><style>'+css+'</style></head><body><a class="skip" href="#content">本文へ移動</a><aside id="sidebar"><div class="brand">REWIND <span>2.0.0</span></div><p class="aside-subtitle">入門とリファレンス</p><label for="search">本文・APIを検索</label><input id="search" type="search" placeholder="例: revert begin / numeric.solve" autocomplete="off" aria-controls="search-results"><p id="search-status" role="status" aria-live="polite"></p><ol id="search-results"></ol>'+toc+'</aside><main id="content"><header class="cover"><div class="tools"><button id="nav-toggle" type="button" aria-expanded="false" aria-controls="sidebar">目次</button><button id="print" type="button">印刷 / PDF保存</button></div><p class="eyebrow">REWIND LANGUAGE / VERSION 2.0.0</p><h1>REWIND<br><span>入門とリファレンス</span></h1><p class="lead">最初の一行から、巻き戻せる状態と外部処理の設計へ。</p><div class="book-facts"><span>32章</span><span>60実行例</span><span>86モジュール</span><span>625標準関数</span></div><p class="cover-note">2026年10月版 · オフライン対応 · 印刷対応</p></header>'+print_toc+'<div class="prose">'+prose+'</div>'+''.join(api)+'<footer>REWIND 2.0.0 入門とリファレンス · <a href="#content">先頭へ</a></footer></main><script type="application/json" id="search-index">'+payload+'</script><script>'+js+'</script></body></html>\n'
(BOOK/"index.html").write_text(document)
manifest={"chapters":chapter,"sections":len(headings)-chapter,"examples":len(seen_examples),"modules":len(data["modules"]),"functions":function_count,"prose_characters":len(text),"html_bytes":len(document.encode()),"search_records":len(search)}
(BOOK/"build-info.json").write_text(json.dumps(manifest,ensure_ascii=False,indent=2)+"\n")
print(json.dumps(manifest,ensure_ascii=False))
