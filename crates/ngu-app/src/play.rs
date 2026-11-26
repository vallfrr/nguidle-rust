//! Plays the game from the prepared data.

use ngu_prep::dirs;
use std::path::Path;

/// Opens the game window on the prepared data in `root`. Saves go to `root/saves`.
pub fn run(root: &Path) {
    let data = dirs::prepared_dir(root);
    let dll = data.join("Assembly-CSharp.dll");
    let saves = std::env::var("NGU_SAVE_DIR")
        .unwrap_or_else(|_| dirs::saves_dir(root).to_string_lossy().into_owned());
    let mut vm = clr::Vm::new(&dll.to_string_lossy());
    vm.eng.screen = (960, 600);
    vm.eng.save_dir = saves;
    let _ = std::fs::create_dir_all(&vm.eng.save_dir);
    vm.eng.desktop_panels = true;
    clr::unity_api::load_prefs(&mut vm);
    vm.load_scene(&data.to_string_lossy()).expect("scene");
    vm.awake_all();
    vm.enable_visual_motion();
    vm.start_desktop_save_picker().expect("save picker");
    let vm: &'static mut clr::Vm = Box::leak(Box::new(vm));
    ngu_desktop::gl::run_gl(
        vm,
        ngu_desktop::Options {
            height: 720,
            fps: 60,
            title: Some(crate::NAME.to_owned()),
            ..Default::default()
        },
    );
}
