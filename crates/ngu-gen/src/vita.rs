//! Translated game logic for the Vita C host.
#![cfg(target_os = "vita")]
#![allow(
    non_camel_case_types, non_snake_case, dead_code, unused_imports, unused_variables,
    unused_mut, unreachable_code, unused_parens, unused_assignments, static_mut_refs
)]

mod ext {
    pub use ngu_rt::ext::*;
    include!("ext_stubs.rs");
}
mod gen;

use ngu_host::clr::{capi, Vm};
use std::os::raw::c_char;

fn attach(vm: &mut Vm) {
    ngu_host::install_panic_hook();
    ngu_host::set_vm(vm as *mut Vm);
    vm.native_mode = true;
    vm.native_inv = Some(gen::hook);
    vm.eng.read_live_fields = Some(ngu_host::live_fields);
}

#[no_mangle]
pub extern "C" fn ngu_vita_init_native(data_dir: *const c_char, width: i32, height: i32) -> i32 {
    capi::init_vita(data_dir, width, height, attach)
}
