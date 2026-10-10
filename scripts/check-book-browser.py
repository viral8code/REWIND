#!/usr/bin/env python3
"""Check the offline reader in Chromium and optionally produce its print PDF."""
import argparse, importlib.metadata, json
from pathlib import Path
from playwright.sync_api import sync_playwright

ROOT=Path(__file__).resolve().parents[1]
BOOK=ROOT/"docs"/"book"
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument("--chromium",default="/usr/bin/chromium")
parser.add_argument("--pdf",action="store_true")
parser.add_argument("--screenshots",type=Path)
args=parser.parse_args()
document=(BOOK/"index.html").read_text()
checks=[]
def record(name,condition):
    if not condition:raise AssertionError(name)
    checks.append(name)
with sync_playwright() as playwright:
    browser=playwright.chromium.launch(executable_path=args.chromium,args=["--no-sandbox"])
    page=browser.new_page(viewport={"width":1440,"height":1000})
    errors=[];requests=[]
    page.on("pageerror",lambda error:errors.append(str(error)))
    page.on("request",lambda request:requests.append(request.url))
    def load(fragment=""):
        page.goto("about:blank")
        page.set_content(document,wait_until="load")
        if fragment:page.evaluate("(fragment)=>{location.hash=fragment}",fragment)
    load();page.wait_for_function("document.body.classList.contains('js')")
    record("32 chapters",page.locator("nav .chapters li").count()==32)
    record("86 modules",page.locator(".module").count()==86)
    record("625 public functions",page.locator(".function").count()==625)
    record("60 downloadable examples",page.locator(".download").count()==60)
    record("unique anchors",page.evaluate("(()=>{const ids=[...document.querySelectorAll('[id]')].map(x=>x.id);return ids.length===new Set(ids).size})()"))
    missing=page.evaluate("Array.from(document.querySelectorAll('a[href^=\"#\"]')).map(a=>a.getAttribute('href').slice(1)).filter(id=>id&&!document.getElementById(id))")
    record("all internal links resolve",not missing)
    record("desktop has no horizontal overflow",page.evaluate("document.documentElement.scrollWidth<=innerWidth"))
    page.locator("#search").fill("numeric.solve")
    page.wait_for_function("document.querySelector('#search-results a')?.textContent === 'std.numeric.solve'")
    record("exact API search",page.locator("#search-results a").first.inner_text()=="std.numeric.solve")
    page.locator("#search").press("Enter")
    record("search navigates to function",page.evaluate("location.hash==='#fn-numeric-solve'"))
    page.locator("#search").fill("revert begin")
    page.wait_for_function("document.querySelector('#search-results a')?.textContent !== 'std.numeric.solve'")
    record("Japanese/prose search",page.locator("#search-results a").count()>0)
    page.locator("#search").press("Escape")
    record("escape clears results",page.locator("#search-results a").count()==0)
    load("#source-map")
    record("implementation link opens source",page.locator("#source-map").evaluate("(element)=>element.open"))
    load("#example-hello")
    with page.expect_download() as downloaded:
        page.locator("#example-hello .download").click()
    artifact=downloaded.value
    record("download filename",artifact.suggested_filename=="hello.rw")
    record("download preserves source",Path(artifact.path()).read_text()=='Out.println("Hello, REWIND!");\npublish;\n')
    page.locator("#example-hello .copy").click()
    record("copy has success or selection fallback",page.locator("#example-hello .copy").inner_text() in ["コピー済み","コードを選択してコピー"])
    load();page.wait_for_load_state()
    if args.screenshots:
        args.screenshots.mkdir(parents=True,exist_ok=True)
        page.screenshot(path=str(args.screenshots/"desktop.png"))
    page.set_viewport_size({"width":390,"height":844})
    record("mobile has no horizontal overflow",page.evaluate("document.documentElement.scrollWidth<=innerWidth"))
    record("mobile TOC initially collapsed",not page.locator("#sidebar").is_visible())
    page.locator("#nav-toggle").click()
    record("mobile TOC toggle",page.locator("#sidebar").is_visible() and page.locator("#nav-toggle").get_attribute("aria-expanded")=="true")
    page.locator("nav .chapters a").first.click()
    record("mobile navigation closes TOC",not page.locator("#sidebar").is_visible())
    load()
    if args.screenshots:page.screenshot(path=str(args.screenshots/"mobile.png"))
    page.set_viewport_size({"width":1440,"height":1000})
    page.emulate_media(media="print")
    record("print hides reader chrome",not page.locator("#sidebar").is_visible())
    record("print hides long implementations",not page.locator("#source-map").is_visible())
    if args.pdf:
        page.pdf(path=str(BOOK/"REWIND-2.0.0-book.pdf"),format="A4",outline=True,tagged=True,print_background=True,prefer_css_page_size=True,display_header_footer=True,header_template="<span></span>",footer_template='<div style="font-size:8px;width:100%;text-align:center;color:#666">REWIND 2.0.0 · <span class="pageNumber"></span> / <span class="totalPages"></span></div>')
        record("PDF generated",(BOOK/"REWIND-2.0.0-book.pdf").stat().st_size>100000)
    record("no JavaScript exceptions",not errors)
    record("no network requests",all(not url.startswith(("http:","https:")) for url in requests))
    nojs=browser.new_context(java_script_enabled=False,viewport={"width":390,"height":844})
    fallback=nojs.new_page();fallback.set_content(document,wait_until="load")
    record("no-JavaScript prose readable",fallback.locator("#ch01").is_visible())
    record("no-JavaScript TOC readable",fallback.locator("#sidebar").is_visible())
    record("no-JavaScript all API present",fallback.locator(".function").count()==625)
    browser.close()
report={"browser":"Chromium","playwright":importlib.metadata.version("playwright"),"checks":checks,"passed":len(checks),"pdf_generated":args.pdf}
(BOOK/"browser-validation.json").write_text(json.dumps(report,ensure_ascii=False,indent=2)+"\n")
print(f"{len(checks)} browser checks passed")
