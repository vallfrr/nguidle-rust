use super::*;

pub(super) fn lookup(key: &str) -> Option<NativeFn> {
    let function: NativeFn = match key {
        // ------------------------------------------------------- Texture / Sprite (generes a l'execution)
        "UnityEngine.Texture2D::.ctor" | "UnityEngine.Texture2D::SetPixel" | "UnityEngine.Texture2D::Apply" | "UnityEngine.ImageConversion::EncodeToPNG" => stub0!(),
        "UnityEngine.Sprite::Create" => stub0!(),
        "UnityEngine.AudioSource::set_mute" => stub0!(),
        "UnityEngine.AudioSource::set_clip" => |vm, a| {
            if let Value::Obj(o) = &a[0] {
                let id = Rc::as_ptr(o) as usize;
                if let Some(name) = with_comp(&a[1], |c| match &c.k {
                    CK::AudioClip(name) => Some(name.clone()),
                    _ => None,
                }).flatten() {
                    vm.eng.audio_clips.insert(id, name);
                } else {
                    vm.eng.audio_clips.remove(&id);
                }
            }
            Ok(Value::Null)
        },
        "UnityEngine.AudioSource::Play" => |vm, a| {
            if let Value::Obj(o) = &a[0] {
                if let Some(name) = vm.eng.audio_clips.get(&(Rc::as_ptr(o) as usize)) {
                    vm.eng.sound_queue.push(name.clone());
                }
            }
            Ok(Value::Null)
        },
        "UnityEngine.Matrix4x4::TRS" | "UnityEngine.GUI::set_matrix" => stub0!(),
        // ------------------------------------------------------- Network (offline)
        "UnityEngine.WWW::.ctor" => stub0!(),
        "UnityEngine.WWW::get_isDone" => |_vm, _a| Ok(Value::I32(1)),
        "UnityEngine.WWW::get_error" => |_vm, _a| Ok(Value::str("offline")),
        "UnityEngine.WWW::get_text" => |_vm, _a| Ok(Value::str("")),
        "UnityEngine.WWWForm::.ctor" | "UnityEngine.WWWForm::AddField" => stub0!(),
        // ------------------------------------------------------- Steam / Kartridge (stubs)
        "Steamworks.SteamUserStats::SetAchievement" => |_vm, _a| Ok(Value::bool(false)),
        k if k.starts_with("Steamworks.") || k.starts_with("Kongregate.") => {
            if k.ends_with("::Init") || k.ends_with("KongregateAPI_IsReady") {
                |_vm, _a| Ok(Value::I32(0))
            } else if k.ends_with("RestartAppIfNecessary") {
                |_vm, _a| Ok(Value::I32(0))
            } else if k.ends_with("GetPersonaName") || k.ends_with("GetCurrentGameLanguage") || k.ends_with("GetUsername") {
                |_vm, _a| Ok(Value::str("english"))
            } else {
                |_vm, _a| Ok(Value::Null)
            }
        }
        _ => return None,
    };
    Some(function)
}
