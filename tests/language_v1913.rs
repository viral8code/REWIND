use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root() -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "rewind-v1913-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&p).unwrap();
    p
}
fn call(r: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(r)
        .args(args)
        .output()
        .unwrap()
}
fn ok(o: &Output) {
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
}
const TAKE:&str="fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic(\"unexpected error\");}}}";
const LEGACY: &str = r#"
fn itemJson(w:Widget,focused:Bool,scroll:Int)->Json effects {} {
 let m=Map<String,Json>();m.set("id",Json::Text(w.id));m.set("kind",Json::Text(w.kind));m.set("x",Json::Int(w.x));m.set("y",Json::Int(w.y));m.set("width",Json::Int(w.width));m.set("height",Json::Int(w.height));m.set("text",Json::Text(w.caption));m.set("foreground",Json::Int(w.foreground));m.set("background",Json::Int(w.background));m.set("enabled",Json::Bool(w.enabled));m.set("checked",Json::Bool(w.checked));m.set("focused",Json::Bool(focused));m.set("cursor",Json::Int(w.cursor));m.set("anchor",Json::Int(w.anchor));m.set("scroll",Json::Int(scroll));return Json::Object(freeze(m));
}

pub fn referenceScene(view:&View)->Result<String,GuiError> effects {} {
 let items=List<Json>();for i in 0..view.widgets.len(){let w=view.widgets.get(i);var scroll=0;match view.scroll.get(w.id){Some(n)=>{scroll=n;},None=>{}}items.add(itemJson(w,view.focus==i,scroll));}
 let m=Map<String,Json>();m.set("title",Json::Text(view.title));m.set("width",Json::Int(view.width));m.set("height",Json::Int(view.height));m.set("background",Json::Int(view.background));m.set("items",Json::Array(freeze(items)));
 match jsonStringify(Json::Object(freeze(m))){Ok(s)=>{return Ok(s);},Err(e)=>{return Err(StdError(e.code,e.offset));}}
}
"#;
fn fixture(source: &str) -> PathBuf {
    let r = root();
    fs::write(
        r.join("reference_gui.rw"),
        format!("{}{LEGACY}", include_str!("../libraries/std/gui.rw")),
    )
    .unwrap();
    fs::write(
        r.join("main.rw"),
        format!("import reference_gui as gui;{TAKE}{source}"),
    )
    .unwrap();
    r
}
#[test]
fn native_scene_matches_reference_bytes_for_controls_focus_selection_unicode_and_revert() {
    let r = fixture(
        r#"let panel=take(gui.window("日本 \"\\",640,320));take(gui.textArea(&mut panel,"body","é\n日本\t\"\\",10,10,300,100));take(gui.checkbox(&mut panel,"flag","Check",true,10,120,100,30));take(gui.button(&mut panel,"button","Button",10,160,100,30));take(gui.rectangle(&mut panel,"rect",120,160,40,30,0xff0011));take(gui.label(&mut panel,"label","A",10,210,120,30));take(gui.setSelection(&mut panel,"body",2,0));take(gui.setScrollPosition(&mut panel,"body",1));take(gui.focus(&mut panel,"body"));assert_eq(gui.scene(&panel),gui.referenceScene(&panel));commit saved;take(gui.style(&mut panel,"body",0x102030,0xabcdef));take(gui.setChecked(&mut panel,"flag",false));take(gui.remove(&mut panel,"label"));take(gui.setEnabled(&mut panel,"button",false));assert_eq(gui.scene(&panel),gui.referenceScene(&panel));revert saved;assert_eq(gui.scene(&panel),gui.referenceScene(&panel));drop saved;take(gui.resize(&mut panel,64,64));assert_eq(gui.scene(&panel),gui.referenceScene(&panel));Out.println(1);publish;"#,
    );
    ok(&call(&r, &["compile", "main.rw"]));
    fs::remove_file(r.join("main.rw")).unwrap();
    fs::remove_file(r.join("reference_gui.rw")).unwrap();
    for mode in ["debug", "compact"] {
        let out = call(
            &r,
            &[
                "run",
                "main.rwc",
                "--record",
                "trace.json",
                "--record-mode",
                mode,
            ],
        );
        ok(&out);
        assert_eq!(out.stdout, b"1\n");
        let replay = call(&r, &["replay", "trace.json"]);
        ok(&replay);
        assert_eq!(out.stdout, replay.stdout);
    }
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn native_scene_rejects_the_same_byte_limit_before_allocating_a_large_output() {
    let r = fixture(
        r#"let panel=take(gui.window("Limit",640,320));var text="";for i in 0..4096{text+="x";}for i in 0..260{take(gui.label(&mut panel,i.format(),text,0,0,10,10));}assert_eq(gui.scene(&panel),Err(StdError("ByteLimit",0)));assert_eq(gui.scene(&panel),gui.referenceScene(&panel));take(gui.setText(&mut panel,"0","small"));assert_eq(gui.scene(&panel),Err(StdError("ByteLimit",0)));"#,
    );
    let out = call(
        &r,
        &[
            "run",
            "main.rw",
            "--history-memory",
            "64MiB",
            "--steps",
            "20000000",
            "--native-work",
            "100000000",
        ],
    );
    ok(&out);
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn native_scene_work_is_charged_and_wrong_widget_shapes_cannot_be_serialized() {
    let r = root();
    fs::write(r.join("main.rw"),format!(r#"{TAKE}let items=List<Int>();items.add(7);let scroll=Map<String,Int>();assert_eq(stdGuiScene("Scene",64,64,0,&items,-1,&scroll),Err(StdError("GuiInvalidScene",0)));"#)).unwrap();
    ok(&call(&r, &["run", "main.rw"]));
    fs::write(r.join("main.rw"),format!(r#"{TAKE}import std.gui as gui;let panel=take(gui.window("Work",320,240));take(gui.label(&mut panel,"one","Hello",0,0,100,20));take(gui.scene(&panel));"#)).unwrap();
    let out = call(&r, &["run", "main.rw", "--native-work", "1"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("NativeWorkBudgetExceeded"));
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn native_scene_is_versioned_without_reinterpreting_old_source_artifacts() {
    let r = root();
    fs::write(
        r.join("rewind.toml"),
        "language = \"1.9.12\"\nsource_root = \".\"\nentry = \"main.rw\"\neffects = \"output\"\n",
    )
    .unwrap();
    fs::write(r.join("main.rw"),"let items=List<Int>();let scroll=Map<String,Int>();stdGuiScene(\"Scene\",64,64,0,&items,-1,&scroll);").unwrap();
    ok(&call(&r, &["update"]));
    let denied = call(&r, &["check"]);
    assert!(!denied.status.success());
    assert!(String::from_utf8_lossy(&denied.stderr).contains("requires language 1.9.13"));
    fs::write(r.join("main.rw"),"let map=Map<String,Json>();map.set(\"width\",Json::Int(64));match jsonStringify(Json::Object(freeze(map))){Ok(s)=>{Out.println(s);},Err(_)=>{panic(\"old serializer\");}}publish;").unwrap();
    ok(&call(&r, &["compile", "main.rw"]));
    fs::remove_file(r.join("main.rw")).unwrap();
    fs::remove_file(r.join("rewind.toml")).unwrap();
    fs::remove_file(r.join("rewind.lock")).unwrap();
    let out = call(&r, &["run", "main.rwc"]);
    ok(&out);
    assert_eq!(out.stdout, b"{\"width\":64}\n");
    fs::remove_dir_all(r).unwrap();
}
