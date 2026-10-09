use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root(source: &str) -> PathBuf {
    let r = std::env::temp_dir().join(format!(
        "rewind-v1959-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&r).unwrap();
    fs::write(r.join("main.rw"), source).unwrap();
    r
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
fn python() -> &'static str {
    if cfg!(windows) {
        "python"
    } else {
        "python3"
    }
}
const COMMON: &str = r#"import std.gui as gui;import std.guiMenu as menu;
fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic("failed");}}}
"#;
#[test]
fn overlay_preserves_text_scroll_and_focus_without_changing_input_views() {
    let source = format!(
        r#"{COMMON}
let base=take(gui.window("Overlay",320,200));take(gui.textArea(&mut base,"body","first\nsecond",0,30,320,100));take(gui.setScrollPosition(&mut base,"body",1));take(gui.focus(&mut base,"body"));
let layer=take(gui.window("layer",320,200));take(gui.button(&mut layer,"menu","Menu",0,0,80,24));take(gui.focus(&mut layer,"menu"));let result=take(gui.overlay(&base,&layer));assert_eq(gui.focused(&result),Some("menu"));assert_eq(take(gui.scrollPosition(&result,"body")),1);assert_eq(gui.focused(&base),Some("body"));assert_eq(take(gui.text(&result,"body")),"first\nsecond");
match gui.overlay(&base,&base){{Err(e)=>{{assert_eq(e.code,"GuiDuplicateId");}},_=>{{panic("duplicate");}}}}
take(gui.graphemeEditing(&mut layer,true));match gui.overlay(&base,&layer){{Err(e)=>{{assert_eq(e.code,"GuiEditingModeMismatch");}},_=>{{panic("editing mode");}}}}assert_eq(gui.bounds(&result).width,320);Out.println(true);publish;"#
    );
    let r = root(&source);
    let o = call(&r, &["run", "main.rw"]);
    ok(&o);
    assert_eq!(o.stdout, b"true\n");
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn menu_viewport_and_disabled_navigation_match_independent_index_sequence() {
    let source = format!(
        r#"{COMMON}
let base=take(gui.window("Scroll menu",320,200));take(gui.button(&mut base,"body","Body",0,170,100,24));take(gui.focus(&mut base,"body"));let choices=menu.create();for i in 0..64{{let id=i.format();take(menu.add(&mut choices,menu.Item(id,"Item "+id,"",i%3==0,false,false)));}}
take(menu.open(&mut choices,10,24,200,gui.bounds(&base)));assert_eq(menu.selected(&choices),Some("0"));
for i in 1..22{{menu.dispatch(&mut choices,gui.Event("key",0,0,"Down",0,0));assert_eq(menu.selected(&choices),Some((i*3).format()));}}let view=take(menu.draw(&choices,&base,"popup"));assert_eq(gui.focused(&view),Some("popup:item:63"));assert_eq(gui.focused(&base),Some("body"));
menu.dispatch(&mut choices,gui.Event("key",0,0,"Down",0,0));assert_eq(menu.selected(&choices),Some("0"));menu.dispatch(&mut choices,gui.Event("key",0,0,"End",0,0));assert_eq(menu.selected(&choices),Some("63"));menu.dispatch(&mut choices,gui.Event("key",0,0,"Home",0,0));assert_eq(menu.selected(&choices),Some("0"));
let old=menu.selected(&choices);match menu.open(&mut choices,0,0,100,gui.Rect(0,0,-9223372036854775808,200)){{Err(e)=>{{assert_eq(e.code,"GuiMenuBounds");}},_=>{{panic("overflow bounds");}}}}assert_eq(menu.selected(&choices),old);
for i in 0..64{{take(menu.setEnabled(&mut choices,i.format(),false));}}assert_eq(menu.selected(&choices),None);let ignored=menu.dispatch(&mut choices,gui.Event("key",0,0,"Enter",0,0));assert(ignored.consumed);assert_eq(ignored.command,None);let unfocused=take(menu.draw(&choices,&base,"popup"));assert_eq(gui.focused(&unfocused),Some("body"));
take(menu.setEnabled(&mut choices,"63",true));assert_eq(menu.selected(&choices),Some("63"));menu.dispatch(&mut choices,gui.Event("key",0,0,"Enter",0,0));assert(!menu.isOpen(&choices));take(menu.open(&mut choices,10,24,200,gui.bounds(&base)));
menu.dispatch(&mut choices,gui.Event("pointer",319,199,"",0,0));assert(!menu.isOpen(&choices));Out.println(true);publish;"#
    );
    let r = root(&source);
    let o = call(&r, &["run", "main.rw"]);
    ok(&o);
    assert_eq!(o.stdout, b"true\n");
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn menu_errors_are_typed_and_do_not_replace_existing_command_state() {
    let source = format!(
        r#"{COMMON}
let choices=menu.create();match menu.open(&mut choices,0,0,100,gui.Rect(0,0,320,200)){{Err(e)=>{{assert_eq(e.code,"GuiMenuEmpty");}},_=>{{panic("empty");}}}}
take(menu.add(&mut choices,menu.Item("toggle","Toggle","Ctrl+T",true,true,false)));
match menu.add(&mut choices,menu.Item("toggle","Other","",true,true,true)){{Err(e)=>{{assert_eq(e.code,"GuiMenuId");}},_=>{{panic("duplicate");}}}}assert(!take(menu.checked(&choices,"toggle")));
match menu.add(&mut choices,menu.Item("invalid","Invalid","",true,false,true)){{Err(e)=>{{assert_eq(e.code,"GuiMenuState");}},_=>{{panic("state");}}}}
for i in 1..256{{take(menu.add(&mut choices,menu.Item(i.format(),"Item","",false,false,false)));}}match menu.add(&mut choices,menu.Item("overflow","Item","",false,false,false)){{Err(e)=>{{assert_eq(e.code,"GuiMenuLimit");}},_=>{{panic("limit");}}}}
let action=menu.dispatch(&mut choices,gui.Event("key",0,0,"Ctrl+T",0,0));assert(action.consumed);assert(take(menu.checked(&choices,"toggle")));match menu.setEnabled(&mut choices,"missing",true){{Err(e)=>{{assert_eq(e.code,"GuiMenuId");}},_=>{{panic("missing");}}}}assert(take(menu.checked(&choices,"toggle")));take(menu.setChecked(&mut choices,"toggle",false));assert(!take(menu.checked(&choices,"toggle")));match menu.setChecked(&mut choices,"1",true){{Err(e)=>{{assert_eq(e.code,"GuiMenuNotCheckable");}},_=>{{panic("unchecked item");}}}}match menu.setChecked(&mut choices,"missing",true){{Err(e)=>{{assert_eq(e.code,"GuiMenuId");}},_=>{{panic("unknown item");}}}}assert(!take(menu.checked(&choices,"toggle")));Out.println(true);publish;"#
    );
    let r = root(&source);
    let o = call(&r, &["run", "main.rw"]);
    ok(&o);
    assert_eq!(o.stdout, b"true\n");
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn menu_fixture_checkpoint_is_source_free_in_both_trace_modes() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for mode in ["debug", "compact"] {
        let r = root(&fs::read_to_string(repo.join("examples/gui-menu/main.rw")).unwrap());
        fs::write(r.join("events.json"),r#"[{"window":"main","event":{"kind":"key","x":0,"y":0,"key":"Ctrl+T","width":0,"height":0}},{"window":"main","event":{"kind":"key","x":0,"y":0,"key":"Ctrl+T","width":0,"height":0}},{"window":"main","event":{"kind":"key","x":0,"y":0,"key":"Ctrl+O","width":0,"height":0}}]"#).unwrap();
        ok(&call(&r, &["compile", "main.rw", "--allow-effects", "gui"]));
        fs::remove_file(r.join("main.rw")).unwrap();
        fs::remove_dir_all(r.join(".rewind")).unwrap();
        let o = call(
            &r,
            &[
                "run",
                "main.rwc",
                "--allow-effects",
                "gui",
                "--gui-window-events",
                "events.json",
                "--record",
                "trace.json",
                "--record-mode",
                mode,
            ],
        );
        ok(&o);
        assert_eq!(o.stdout, b"ready\ntoggle\nundo\ntoggle\nopen\n");
        fs::remove_file(r.join("events.json")).unwrap();
        let replay = call(&r, &["replay", "trace.json", "--allow-effects", "gui"]);
        ok(&replay);
        assert_eq!(replay.stdout, o.stdout);
        fs::remove_dir_all(r).unwrap();
    }
}
#[test]
fn native_menu_commands_restore_and_replay_without_a_display() {
    if cfg!(target_os = "linux") && std::env::var_os("DISPLAY").is_none() {
        return;
    }
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let r = std::env::temp_dir().join(format!("rewind-native-menu-{}", std::process::id()));
    let result = Command::new(python())
        .args([
            repo.join("scripts/smoke-gui-menu-sdk.py").as_os_str(),
            PathBuf::from(env!("CARGO_BIN_EXE_rewind")).as_os_str(),
            r.as_os_str(),
            repo.join("examples/gui-menu/main.rw").as_os_str(),
        ])
        .output()
        .unwrap();
    ok(&result);
    fs::remove_dir_all(r).unwrap();
}
