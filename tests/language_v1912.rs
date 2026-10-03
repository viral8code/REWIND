use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn root() -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "rewind-v1912-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&p).unwrap();
    p
}
fn call(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rewind"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap()
}
fn ok(out: &Output) {
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
const TAKE:&str="fn take<T,E>(r:Result<T,E>)->T effects {} {match move r{Ok(v)=>{return move v;},Err(_)=>{panic(\"unexpected error\");}}}";
fn execute(source: &str) {
    let r = root();
    fs::write(r.join("main.rw"), format!("{TAKE}{source}")).unwrap();
    ok(&call(
        &r,
        &[
            "run",
            "main.rw",
            "--steps",
            "20000000",
            "--native-work",
            "100000000",
        ],
    ));
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn forms_validate_all_fields_and_convert_types_without_losing_incomplete_input() {
    execute(
        r#"
 import std.guiForm as form;
 let schema=List<form.Field>();schema.add(form.Field("integer","Int",form.Kind::Integer(-10,10),true,32,"-"));schema.add(form.Field("real","Float",form.Kind::Real(-2.0,2.0),true,32,"nan"));schema.add(form.Field("stamp","Date",form.Kind::Timestamp,true,64,"bad"));schema.add(form.Field("optional","Optional",form.Kind::Integer(0,10),false,32,""));schema.add(form.Field("agree","Agree",form.Kind::Check,true,5,"false"));
 let model=take(form.create(&schema,3));assert(!form.validate(&mut model));assert_eq(take(form.value(&model,"integer")),"-");assert_eq(take(form.error(&model,"integer")),Some("GuiFormInteger"));assert_eq(take(form.error(&model,"real")),Some("GuiFormReal"));assert_eq(take(form.error(&model,"stamp")),Some("GuiFormTimestamp"));assert_eq(take(form.error(&model,"agree")),Some("GuiFormRequired"));
 match form.toJson(&model){Err(e)=>{assert_eq(e.offset,0);},Ok(_)=>{panic("invalid form accepted");}}
 take(form.setValue(&mut model,"integer"," 10 "));take(form.setValue(&mut model,"real","1.5"));take(form.setValue(&mut model,"stamp","2024-02-29T12:34:56+09:00"));take(form.setValue(&mut model,"agree","true"));assert(form.validate(&mut model));
 let json=take(form.toJson(&model));assert_eq(jsonGet(json,"integer"),Some(Json::Int(10)));assert_eq(jsonGet(json,"real"),Some(Json::Float(1.5)));assert_eq(jsonGet(json,"stamp"),Some(Json::Text("2024-02-29T03:34:56Z")));assert_eq(jsonGet(json,"optional"),Some(Json::Null));assert_eq(jsonGet(json,"agree"),Some(Json::Bool(true)));
 take(form.setValue(&mut model,"integer","11"));assert(!form.validate(&mut model));assert_eq(take(form.error(&model,"integer")),Some("GuiFormRange"));form.reset(&mut model);assert_eq(take(form.value(&model,"integer")),"-");assert_eq(take(form.error(&model,"integer")),None);
 "#,
    );
}
#[test]
fn form_edit_limits_graphemes_focus_and_revert_survive_rebuilding_views() {
    execute(
        r#"
 import std.guiForm as form;import std.gui as gui;
 let schema=List<form.Field>();schema.add(form.Field("text","Text",form.Kind::Text,true,6,"é"));schema.add(form.Field("flag","Flag",form.Kind::Check,false,5,"false"));let model=take(form.create(&schema,2));var panel=take(form.render(&model,"Input",320,240,72,"f"));
 take(form.dispatch(&mut model,&mut panel,gui.Event("pointer",20,40,"",0,0),"f"));assert_eq(form.dispatch(&mut model,&mut panel,gui.Event("text",0,0,"日本",0,0),"f"),Err(StdError("GuiInvalidText",0)));assert_eq(take(form.value(&model,"text")),"é");assert_eq(take(gui.text(&panel,"f:field:text")),"é");assert_eq(take(gui.selection(&panel,"f:field:text")),(2,2));
 commit saved;take(form.dispatch(&mut model,&mut panel,gui.Event("key",0,0,"Backspace",0,0),"f"));assert_eq(take(form.value(&model,"text")),"");panel=take(form.render(&model,"Input",320,240,72,"f"));assert_eq(gui.focused(&panel),Some("f:field:text"));take(form.dispatch(&mut model,&mut panel,gui.Event("text",0,0,"日",0,0),"f"));assert_eq(take(form.value(&model,"text")),"日");revert saved;assert_eq(take(form.value(&model,"text")),"é");drop saved;
 take(form.dispatch(&mut model,&mut panel,gui.Event("key",0,0,"Tab",0,0),"f"));panel=take(form.render(&model,"Input",320,240,72,"f"));assert_eq(gui.focused(&panel),Some("f:field:flag"));take(form.dispatch(&mut model,&mut panel,gui.Event("key",0,0,"Space",0,0),"f"));assert_eq(take(form.value(&model,"flag")),"true");
 "#,
    );
}
#[test]
fn form_textarea_scroll_bounds_are_preserved_in_the_model() {
    execute(
        r#"
 import std.guiForm as form;import std.gui as gui;
 let schema=List<form.Field>();schema.add(form.Field("body","Body",form.Kind::Multiline,false,64,"a\nb\nc\nd"));schema.add(form.Field("other","Other",form.Kind::Text,false,64,""));let model=take(form.create(&schema,1));var panel=take(form.render(&model,"Text",320,240,96,"f"));take(form.dispatch(&mut model,&mut panel,gui.Event("pointer",20,40,"",0,0),"f"));take(form.dispatch(&mut model,&mut panel,gui.Event("wheel",0,9223372036854775807,"",0,0),"f"));assert_eq(form.position(&model),0);assert_eq(take(gui.scrollPosition(&panel,"f:field:body")),3);panel=take(form.render(&model,"Text",320,240,96,"f"));assert_eq(take(gui.scrollPosition(&panel,"f:field:body")),3);take(form.dispatch(&mut model,&mut panel,gui.Event("wheel",0,-9223372036854775807-1,"",0,0),"f"));assert_eq(take(gui.scrollPosition(&panel,"f:field:body")),0);
 "#,
    );
}
#[test]
fn tables_share_large_data_and_clamp_against_an_independent_viewport_model() {
    let mut s = String::from(
        r#"import std.guiTable as table;import std.gui as gui;let heads=List<String>();heads.add("a");heads.add("b");heads.add("c");let cells=List<String>();for i in 0..3000{cells.add(i.format());}let grid=take(table.create(freeze(heads),freeze(cells),4));"#,
    );
    let mut first = 0i64;
    let mut seed = 17u64;
    for step in 0..200 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        if step % 3 == 0 {
            let row = (seed % 1000) as i64;
            if row < first {
                first = row;
            }
            if row >= first + 4 {
                first = row - 3;
            }
            s+=&format!("take(table.select(&mut grid,{row}));assert_eq(table.selection(&grid),Some({row}));");
        } else {
            let delta = if step == 1 {
                i64::MAX
            } else if step == 2 {
                i64::MIN
            } else {
                (seed % 3001) as i64 - 1500
            };
            first = (first as i128 + delta as i128).clamp(0, 996) as i64;
            s += &format!("table.scroll(&mut grid,{delta});");
        }
        s += &format!("assert_eq(table.position(&grid),{first});");
    }
    s += r#"table.scroll(&mut grid,-9223372036854775807-1);let panel=take(table.render(&grid,"Table",640,240,30,"表"));assert_eq(take(table.dispatch(&mut grid,&mut panel,gui.Event("pointer",20,80,"",0,0),"表")),Some(table.Action::Selected(1)));assert_eq(table.cell(&grid,1,2),Ok("5"));let scene=take(jsonParse(take(gui.scene(&panel))));match jsonGet(scene,"items"){Some(Json::Array(items))=>{assert_eq(items.len(),19);},_=>{panic("missing visible rows");}}commit selected;take(table.select(&mut grid,999));revert selected;assert_eq(table.selection(&grid),Some(1));drop selected;"#;
    execute(&s);
}
#[test]
fn controls_remove_preserves_selection_scroll_and_other_focus_and_errors_are_atomic() {
    execute(
        r#"
 import std.gui as gui;let panel=take(gui.window("Controls",640,320));take(gui.checkbox(&mut panel,"first","Check",false,10,10,100,30));take(gui.textArea(&mut panel,"body","a\nb\nc",10,50,200,100));take(gui.button(&mut panel,"last","Last",10,170,100,30));take(gui.setSelection(&mut panel,"body",2,1));take(gui.setScrollPosition(&mut panel,"body",1));take(gui.focus(&mut panel,"body"));take(gui.setChecked(&mut panel,"first",true));assert_eq(gui.checked(&panel,"first"),Ok(true));
 let before=take(gui.scene(&panel));assert_eq(gui.remove(&mut panel,"absent"),Err(StdError("GuiUnknownId",0)));assert_eq(gui.style(&mut panel,"body",-1,0),Err(StdError("GuiInvalidColor",0)));assert_eq(gui.setScrollPosition(&mut panel,"body",3),Err(StdError("GuiInvalidScroll",0)));assert_eq(take(gui.scene(&panel)),before);
 take(gui.remove(&mut panel,"first"));assert_eq(gui.focused(&panel),Some("body"));assert_eq(gui.selection(&panel,"body"),Ok((2,1)));assert_eq(gui.scrollPosition(&panel,"body"),Ok(1));take(gui.setEnabled(&mut panel,"body",false));assert_eq(gui.focused(&panel),None);take(gui.remove(&mut panel,"body"));assert_eq(gui.focused(&panel),None);assert_eq(gui.dispatch(&mut panel,gui.Event("pointer",20,180,"",0,0)),Some(gui.Action::Activate("last")));
 "#,
    );
}
#[test]
fn forms_and_tables_source_free_multi_window_input_undo_and_replay() {
    let r = root();
    fs::write(
        r.join("main.rw"),
        include_str!("../examples/gui-controls/main.rw"),
    )
    .unwrap();
    ok(&call(&r, &["compile", "main.rw", "--allow-effects", "gui"]));
    fs::remove_file(r.join("main.rw")).unwrap();
    for mode in ["compact"] {
        fs::write(
            r.join("events.json"),
            include_str!("../examples/gui-controls/events.json"),
        )
        .unwrap();
        let out = call(
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
        ok(&out);
        assert_eq!(
            out.stdout,
            "{\"active\":false,\"age\":13,\"name\":\"日本語\"}\n1\n日本\n".as_bytes()
        );
        fs::remove_file(r.join("events.json")).unwrap();
        let replay = call(&r, &["replay", "trace.json", "--allow-effects", "gui"]);
        ok(&replay);
        assert_eq!(replay.stdout, out.stdout);
    }
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn native_gui_surface_forms_and_tables_present_and_close_without_input_fixture() {
    if cfg!(target_os = "linux") && std::env::var_os("DISPLAY").is_none() {
        return;
    }
    if !cfg!(any(target_os = "linux", target_os = "windows")) {
        return;
    }
    let r = root();
    fs::write(r.join("main.rw"),format!(r#"{TAKE}import std.guiForm as form;import std.guiTable as table;import std.guiWindows as windows;let defs=List<form.Field>();defs.add(form.Field("name","Name",form.Kind::Text,true,64,"REWIND"));let model=take(form.create(&defs,1));let headings=List<String>();headings.add("Value");let data=List<String>();data.add("one");let grid=take(table.create(freeze(headings),freeze(data),1));let input=take(form.render(&model,"Form",320,240,72,"f"));let rows=take(table.render(&grid,"Rows",320,240,30,"t"));take(windows.present("form",&input));take(windows.present("table",&rows));publish;take(windows.close("form"));take(windows.close("table"));publish;Out.println(2);publish;"#)).unwrap();
    ok(&call(&r, &["compile", "main.rw", "--allow-effects", "gui"]));
    fs::remove_file(r.join("main.rw")).unwrap();
    let out = call(&r, &["run", "main.rwc", "--allow-effects", "gui"]);
    ok(&out);
    assert_eq!(out.stdout, b"2\n");
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn imported_nested_enum_patterns_keep_their_own_alias_without_a_caller_import() {
    let r = root();
    fs::write(
        r.join("leaf.rw"),
        "pub enum Choice {Yes(Int),No}pub fn choice()->Choice effects {} {return Choice::Yes(7);}",
    )
    .unwrap();
    fs::write(r.join("wrapper.rw"),"import leaf as privateLeaf;pub fn read()->Int effects {} {match Some(privateLeaf.choice()){Some(privateLeaf.Choice::Yes(value))=>{return value;},_=>{return 0;}}}").unwrap();
    fs::write(r.join("other.rw"),"pub enum Choice {Yes(String),No}pub fn choice()->Choice effects {} {return Choice::Yes(\"unrelated\");}").unwrap();
    fs::write(r.join("main.rw"),"import wrapper as wrapper;import other as privateLeaf;import std.text as text;let text=\"日本\";assert_eq(text.byteLen(),6);assert_eq(text.charLen(),2);assert_eq(wrapper.read(),7);match privateLeaf.choice(){privateLeaf.Choice::Yes(value)=>{assert_eq(value,\"unrelated\");},_=>{panic(\"wrong alias\");}}Out.println(7);publish;").unwrap();
    ok(&call(&r, &["compile", "main.rw"]));
    fs::remove_file(r.join("main.rw")).unwrap();
    fs::remove_file(r.join("leaf.rw")).unwrap();
    fs::remove_file(r.join("wrapper.rw")).unwrap();
    fs::remove_file(r.join("other.rw")).unwrap();
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
        assert_eq!(out.stdout, b"7\n");
        let replay = call(&r, &["replay", "trace.json"]);
        ok(&replay);
        assert_eq!(out.stdout, replay.stdout);
    }
    fs::remove_dir_all(r).unwrap();
}
#[test]
fn schema_empty_table_and_render_failures_preserve_existing_models() {
    execute(
        r#"
 import std.guiForm as form;import std.guiTable as table;
 let defs=List<form.Field>();defs.add(form.Field("id","Caption",form.Kind::Text,true,4,"ok"));let model=take(form.create(&defs,1));assert_eq(form.setValue(&mut model,"id","12345"),Err(StdError("GuiInvalidText",0)));assert_eq(form.setValue(&mut model,"missing","x"),Err(StdError("GuiUnknownId",0)));assert_eq(take(form.value(&model,"id")),"ok");match form.render(&model,"Bad",240,144,128,"f"){Err(e)=>{assert_eq(e.code,"GuiInvalidBounds");},Ok(_)=>{panic("bad geometry");}}
 defs.add(form.Field("id","Duplicate",form.Kind::Text,false,4,""));match form.create(&defs,1){Err(e)=>{assert_eq(e.code,"GuiInvalidId");assert_eq(e.offset,1);},Ok(_)=>{panic("duplicate schema");}}
 let head=List<String>();head.add("Value");let frozenHead=freeze(head);let cells=List<String>();let empty=take(table.create(frozenHead,freeze(cells),4));assert_eq(table.rows(&empty),0);assert_eq(table.selection(&empty),None);assert_eq(table.select(&mut empty,0),Err(StdError("GuiTableRange",0)));table.scroll(&mut empty,9223372036854775807);assert_eq(table.position(&empty),0);take(table.render(&empty,"Empty",640,240,30,"t"));let bad=List<String>();bad.add("a\0b");match table.create(frozenHead,freeze(bad),4){Err(e)=>{assert_eq(e.code,"GuiInvalidText");},Ok(_)=>{panic("NUL accepted");}}
 "#,
    );
}
