//! The controller glyphs of the console mode (`clr/assets/glyphs`, drawn by `tools/glyphs.py` in the game's style), served as
//! textures under the key `port-glyphs/<name>` and used as sprites where the game shows a key prompt.
use crate::unity::*;
use crate::value::Value;
use crate::Vm;

const GLYPHS: &[(&str, &[u8])] = &[
    ("cross", include_bytes!("../assets/glyphs/cross.png")),
    ("cross-lit", include_bytes!("../assets/glyphs/cross-lit.png")),
    ("circle", include_bytes!("../assets/glyphs/circle.png")),
    ("circle-lit", include_bytes!("../assets/glyphs/circle-lit.png")),
    ("square", include_bytes!("../assets/glyphs/square.png")),
    ("square-lit", include_bytes!("../assets/glyphs/square-lit.png")),
    ("triangle", include_bytes!("../assets/glyphs/triangle.png")),
    ("triangle-lit", include_bytes!("../assets/glyphs/triangle-lit.png")),
    ("l", include_bytes!("../assets/glyphs/l.png")),
    ("r", include_bytes!("../assets/glyphs/r.png")),
    ("start", include_bytes!("../assets/glyphs/start.png")),
    ("select", include_bytes!("../assets/glyphs/select.png")),
    ("dpad", include_bytes!("../assets/glyphs/dpad.png")),
    ("stick", include_bytes!("../assets/glyphs/stick.png")),
    ("prompt-advance", include_bytes!("../assets/glyphs/prompt-advance.png")),
    ("prompt-advance-lit", include_bytes!("../assets/glyphs/prompt-advance-lit.png")),
    ("prompt-back", include_bytes!("../assets/glyphs/prompt-back.png")),
    ("prompt-back-lit", include_bytes!("../assets/glyphs/prompt-back-lit.png")),
    ("prompt-both", include_bytes!("../assets/glyphs/prompt-both.png")),
    ("prompt-both-lit", include_bytes!("../assets/glyphs/prompt-both-lit.png")),
];

/// The PNG of a glyph, when the path is `tex/port-glyphs/<name>.png`.
pub(crate) fn embedded(path: &str) -> Option<Vec<u8>> {
    let name = path.strip_prefix("tex/port-glyphs/")?.strip_suffix(".png")?;
    GLYPHS.iter().find(|(n, _)| *n == name).map(|(_, bytes)| bytes.to_vec())
}

fn size_of(bytes: &[u8]) -> (i32, i32) {
    let be = |i: usize| i32::from_be_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]);
    (be(16), be(20))
}

/// A sprite of the glyph, made once.
pub(crate) fn sprite(vm: &mut Vm, name: &str) -> Value {
    if let Some(s) = vm.eng.glyph_sprites.get(name) {
        return s.clone();
    }
    let Some((_, bytes)) = GLYPHS.iter().find(|(n, _)| *n == name) else { return Value::Null };
    let (w, h) = size_of(bytes);
    let key = format!("port-glyphs/{name}");
    let texture = new_comp(vm, "UnityEngine.Texture2D", Value::Null, CK::Texture(Box::new(TexD { name: key.clone(), key: key.clone(), w, h })));
    let sprite = new_comp(vm, "UnityEngine.Sprite", Value::Null, CK::Sprite(Box::new(SpriteD {
        name: key, tex: texture, rect: [0.0, 0.0, w as f32, h as f32], border: [0.0; 4], pivot: [0.5, 0.5], ppu: 100.0,
    })));
    vm.eng.glyph_sprites.insert(name.into(), sprite.clone());
    sprite
}
