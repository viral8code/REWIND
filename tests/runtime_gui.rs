use rewind::{
    gui::{Event, Frame},
    Runtime,
};
use std::{
    fs,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn setup() -> (std::path::PathBuf, Runtime) {
    let p = std::env::temp_dir().join(format!(
        "rewind-gui-runtime-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&p).unwrap();
    let mut rt = Runtime::new(&p).unwrap();
    rt.enable_incremental_publish();
    rt.configure_gui_events(vec![Event::simple("close"), Event::simple("key")])
        .unwrap();
    (p, rt)
}
fn frame(title: &str) -> Frame {
    Frame {
        title: title.into(),
        width: 240,
        height: 160,
        background: 0xffffff,
        items: vec![],
    }
}
fn publish(rt: &mut Runtime) {
    rt.publish(false, &mut vec![], &mut vec![]).unwrap();
}
#[test]
fn published_scenes_survive_revert_and_do_not_republish_old_operations() {
    let (p, mut rt) = setup();
    rt.gui_stage(Some(frame("one"))).unwrap();
    assert!(rt.gui_displayed_frame().is_none());
    rt.commit("staged").unwrap();
    publish(&mut rt);
    rt.commit("one").unwrap();
    rt.gui_stage(Some(frame("two"))).unwrap();
    publish(&mut rt);
    rt.revert("staged").unwrap();
    publish(&mut rt);
    assert_eq!(rt.gui_displayed_frame().unwrap().title, "two");
    rt.revert("one").unwrap();
    rt.gui_stage(Some(frame("one"))).unwrap();
    publish(&mut rt);
    assert_eq!(rt.gui_displayed_frame().unwrap().title, "one");
    rt.gui_stage(None).unwrap();
    rt.revert("one").unwrap();
    publish(&mut rt);
    assert!(rt.gui_displayed_frame().is_some());
    rt.gui_stage(None).unwrap();
    publish(&mut rt);
    assert!(rt.gui_displayed_frame().is_none());
    fs::remove_dir_all(p).unwrap();
}
#[test]
fn event_cursor_rewinds_but_continue_input_never_skips_future_replay_events() {
    let (p, mut rt) = setup();
    rt.gui_stage(Some(frame("one"))).unwrap();
    publish(&mut rt);
    rt.commit("input").unwrap();
    assert_eq!(rt.gui_next_event().unwrap().kind, "close");
    rt.revert("input").unwrap();
    assert_eq!(rt.gui_next_event().unwrap().kind, "close");
    rt.revert("input").unwrap();
    rt.gui_continue_input();
    assert_eq!(rt.gui_next_event().unwrap().kind, "key");
    let tape = rt.export_observations().unwrap();
    let mut replay = Runtime::new(&p).unwrap();
    replay.enable_incremental_publish();
    replay.import_observations(&tape).unwrap();
    replay.gui_stage(Some(frame("one"))).unwrap();
    publish(&mut replay);
    replay.gui_continue_input();
    assert_eq!(replay.gui_next_event().unwrap().kind, "close");
    replay.gui_continue_input();
    assert_eq!(replay.gui_next_event().unwrap().kind, "key");
    assert!(replay.gui_next_event().is_err());
    fs::remove_dir_all(p).unwrap();
}
#[test]
fn reset_begin_discards_pending_scene_and_checkpoints_without_closing_published_window() {
    let (p, mut rt) = setup();
    rt.install_begin().unwrap();
    rt.gui_stage(Some(frame("one"))).unwrap();
    publish(&mut rt);
    rt.commit("user").unwrap();
    rt.gui_stage(None).unwrap();
    rt.taint_checkpoints();
    rt.reset_begin().unwrap();
    assert!(rt.checkpoint_parent("user").is_err());
    publish(&mut rt);
    assert_eq!(rt.gui_displayed_frame().unwrap().title, "one");
    rt.commit("user").unwrap();
    fs::remove_dir_all(p).unwrap();
}
