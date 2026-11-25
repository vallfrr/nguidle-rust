//! Desktop GPU window: `ngu-window <Assembly-CSharp.dll> <out/ngu> [fps] [max_frames]`.
//! `NGU_SAVE_DIR` overrides the save folder (default `<out/ngu>/save`).
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.len() < 2 {
        eprintln!("usage: ngu-window <Assembly-CSharp.dll> <out/ngu>");
        std::process::exit(2);
    }
    let mut vm = clr::Vm::new(&a[0]);
    vm.eng.screen = (960, 600);
    vm.eng.save_dir = std::env::var("NGU_SAVE_DIR").unwrap_or_else(|_| format!("{}/save", a[1]));
    let _ = std::fs::create_dir_all(&vm.eng.save_dir);
    vm.eng.desktop_panels = true;
    clr::unity_api::load_prefs(&mut vm);
    vm.load_scene(&a[1]).expect("scene");
    vm.awake_all();
    vm.enable_visual_motion();
    vm.start_desktop_save_picker().expect("save picker");
    if let Ok(steps) = std::env::var("NGU_STEPS") {
        for _ in 0..40 {
            vm.frame(1.0 / 30.0);
        }
        let steps = steps.split(';').map(str::to_owned).collect::<Vec<_>>();
        clr::script::run_steps(&mut vm, &steps);
    }
    let vm: &'static mut clr::Vm = Box::leak(Box::new(vm));
    ngu_desktop::gl::run_gl(vm, ngu_desktop::Options {
        height: 720,
        fps: a.get(2).and_then(|s| s.parse().ok()).unwrap_or(60),
        max_frames: a.get(3).and_then(|s| s.parse().ok()),
        ..Default::default()
    });
}
