(() => {
  "use strict";
  document.body.classList.add("js");
  const index = JSON.parse(document.getElementById("search-index").textContent);
  const search = document.getElementById("search");
  const results = document.getElementById("search-results");
  const status = document.getElementById("search-status");
  const normalize = text => text.normalize("NFKC").toLocaleLowerCase("ja");
  const records = index.map(item => ({...item, normalized:normalize(item.title+" "+item.text)}));
  let timer;
  function find() {
    const query = normalize(search.value.trim()).slice(0, 160);
    results.replaceChildren();
    if (!query) { status.textContent=""; return; }
    const terms = query.split(/\s+/);
    const found = records.filter(item => terms.every(term => item.normalized.includes(term)))
      .sort((a,b) => Number(normalize(b.title).includes(query))-Number(normalize(a.title).includes(query)));
    status.textContent = found.length ? found.length+"件。先頭12件を表示。" : "見つかりません。型名や短い語で検索してください。";
    for (const item of found.slice(0,12)) {
      const li=document.createElement("li"), link=document.createElement("a"), snippet=document.createElement("small");
      link.href="#"+item.id;link.textContent=item.title;
      const offset=normalize(item.text).indexOf(terms[0]);
      snippet.textContent=item.text.slice(Math.max(0,offset-30),Math.max(0,offset-30)+100).replace(/\s+/g," ");
      li.append(link,snippet);results.append(li);
    }
  }
  search.addEventListener("input",()=>{clearTimeout(timer);timer=setTimeout(find,120)});
  search.addEventListener("keydown",event=>{
    if(event.key==="Enter"){const link=results.querySelector("a");if(link){link.click();event.preventDefault()}}
    if(event.key==="Escape"){search.value="";find()}
  });
  const toggle=document.getElementById("nav-toggle");
  toggle.addEventListener("click",()=>{
    const open=document.body.classList.toggle("nav-open");toggle.setAttribute("aria-expanded",String(open));
    if(open)document.getElementById("sidebar").scrollIntoView();
  });
  document.getElementById("sidebar").addEventListener("click",event=>{
    if(event.target.closest('a[href^="#"]')){
      document.body.classList.remove("nav-open");toggle.setAttribute("aria-expanded","false");
    }
  });
  function openTarget() {
    if(!location.hash)return;
    let target;try{target=document.getElementById(decodeURIComponent(location.hash.slice(1)))}catch{return}
    if(!target)return;
    const detail=target.closest("details");
    if(detail)detail.open=true;
    target.scrollIntoView({block:"start"});
  }
  addEventListener("hashchange",openTarget);
  openTarget();
  document.getElementById("print").addEventListener("click",()=>window.print());
  const restore=[];
  addEventListener("beforeprint",()=>{
    document.querySelectorAll("details.expected").forEach(detail=>{
      restore.push([detail,detail.open]);detail.open=true;
    });
  });
  addEventListener("afterprint",()=>{
    for(const [detail,open] of restore.splice(0))detail.open=open;
  });
  document.addEventListener("click",async event=>{
    const button=event.target.closest("button.copy,button.download");
    if(!button)return;
    const text=button.closest(".codebox").querySelector("pre code").textContent+"\n";
    if(button.classList.contains("download")){
      const url=URL.createObjectURL(new Blob([text],{type:"text/plain;charset=utf-8"}));
      const link=document.createElement("a");link.href=url;link.download=button.dataset.filename;link.click();
      setTimeout(()=>URL.revokeObjectURL(url),1000);return;
    }
    try{
      if(navigator.clipboard && window.isSecureContext)await navigator.clipboard.writeText(text);
      else{
        const field=document.createElement("textarea");field.value=text;field.style.position="fixed";field.style.opacity="0";
        document.body.append(field);field.select();
        const ok=document.execCommand("copy");field.remove();if(!ok)throw new Error("copy unavailable");
      }
      button.textContent="コピー済み";
    }catch{
      button.textContent="コードを選択してコピー";
      const range=document.createRange();range.selectNodeContents(button.closest(".codebox").querySelector("pre code"));
      const selection=window.getSelection();selection.removeAllRanges();selection.addRange(range);
    }
    setTimeout(()=>{button.textContent="コピー"},2000);
  });
})();
