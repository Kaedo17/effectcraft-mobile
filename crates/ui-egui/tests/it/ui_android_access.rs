//! The launch-time "All files access" prompt draws and answers in a real frame
//! (`panels::android_access`). Its grant logic runs in the module's own unit tests; this
//! exercises what only a real frame can: the modal chrome, the two buttons and the tick the
//! draw path calls every frame.
use std::cell::Cell;
use std::rc::Rc;

use effectcraft_engine::Session;
use effectcraft_ui_egui::EffectcraftApp;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

/// A host that can check and ask for the grant; every ask lands in the counter.
fn android_app(granted: bool) -> (EffectcraftApp, Rc<Cell<bool>>, Rc<Cell<u32>>) {
    let mut app = EffectcraftApp::new(Session::default());
    let flag = Rc::new(Cell::new(granted));
    let asks = Rc::new(Cell::new(0));
    let (g, a) = (flag.clone(), asks.clone());
    app.hooks.shared_storage_root = Some("/storage/emulated/0".into());
    app.hooks.shared_storage_granted = Some(Box::new(move || g.get()));
    app.hooks.shared_storage_grant = Some(Box::new(move || a.set(a.get() + 1)));
    (app, flag, asks)
}

fn harness(granted: bool) -> (Harness<'static, EffectcraftApp>, Rc<Cell<bool>>, Rc<Cell<u32>>) {
    let (app, flag, asks) = android_app(granted);
    let h = Harness::builder().with_size(egui::vec2(1000.0, 720.0)).build_eframe(|_| app);
    (h, flag, asks)
}
/// The launch ask appears once the first frames have rendered, and its Grant access button
/// reaches the host's ask (opening Android's settings page through the JNI hook).
#[test]
fn the_launch_prompt_draws_and_grant_access_reaches_the_host() {
    let (mut h, granted, asks) = harness(false);
    h.run_steps(2);
    assert!(!h.state().android_access_open(), "not before the trigger frame");
    h.run_steps(10);
    assert!(h.state().android_access_open(), "asked once the first frames have rendered");
    h.get_by_label("Grant access").click();
    h.run_steps(3);
    assert_eq!(asks.get(), 1, "the ask reaches the host's JNI");
    assert!(!granted.get(), "the grant only holds after the user flips it in Settings");
    assert!(!h.state().android_access_open());
    // One-shot: the prompt never springs back while the run keeps drawing.
    h.run_steps(12);
    assert!(!h.state().android_access_open(), "the launch ask is offered once per run");
    assert!(!h.state().session.prefs.general.android_skip_all_files_access_prompt);
}

/// Not Now answers the prompt without asking and is remembered in the preferences.
#[test]
fn the_launch_prompt_not_now_is_remembered() {
    let (mut h, _, asks) = harness(false);
    h.run_steps(12);
    assert!(h.state().android_access_open(), "asked once the first frames have rendered");
    h.get_by_label("Not Now").click();
    h.run_steps(3);
    assert!(!h.state().android_access_open(), "the prompt closed");
    assert_eq!(asks.get(), 0, "Not Now never reaches the host's ask");
    assert!(h.state().session.prefs.general.android_skip_all_files_access_prompt);
}

/// A host that already holds the grant never asks, and the prompt is never offered again.
#[test]
fn a_held_grant_never_prompts() {
    let (mut h, _, asks) = harness(true);
    h.run_steps(14);
    assert!(!h.state().android_access_open());
    assert_eq!(asks.get(), 0);
}
