use super::*;

// ---------------------------------------------------------------- scene loading
impl Vm {
    /// Loads scene.json and builds all objects (parsed object by object: no global DOM).
    pub fn load_scene(&mut self, dir: &str) -> Result<(), String> {
        self.eng.assets_dir = dir.to_string();
        crate::assets::init(dir);
        let txt = crate::assets::read_string("scene.json").ok_or("scene.json not found")?;
        #[derive(serde::Deserialize)]
        struct Root<'a> {
            #[serde(borrow)]
            objects: indexmap::IndexMap<&'a str, &'a serde_json::value::RawValue>,
            resources: Option<Vec<(String, String)>>,
        }
        let root: Root = serde_json::from_str(&txt).map_err(|e| e.to_string())?;
        let objs = &root.objects;
        fn kind(raw: &serde_json::value::RawValue) -> &str {
            raw.get().strip_prefix("{\"t\":\"").and_then(|value| value.split_once('"')).map_or("", |(name, _)| name)
        }
        let pj = |raw: &serde_json::value::RawValue| -> J { serde_json::from_str(raw.get()).unwrap_or(J::Null) };

        // 1. passive objects: textures, fonts, texts, GameObjects
        for (k, raw) in objs.iter() {
            let t = kind(raw);
            match t {
                "Texture2D" => {
                    let v = pj(raw);
                    let o = new_comp(self, "UnityEngine.Texture2D", Value::Null, CK::Texture(Box::new(TexD { name: v["name"].as_str().unwrap_or("").into(), key: k.to_string(), w: v["w"].as_i64().unwrap_or(0) as i32, h: v["h"].as_i64().unwrap_or(0) as i32 })));
                    self.eng.keys.insert(k.to_string(), o);
                }
                "Font" => {
                    let v = pj(raw);
                    let o = new_comp(self, "UnityEngine.Font", Value::Null, CK::Font { name: v["name"].as_str().unwrap_or("").into(), file: v["file"].as_str().unwrap_or("").into() });
                    self.eng.keys.insert(k.to_string(), o);
                }
                "TextAsset" => {
                    let v = pj(raw);
                    let o = new_comp(self, "UnityEngine.TextAsset", Value::Null, CK::TextAsset {
                        name: v["name"].as_str().unwrap_or("").into(), key: k.to_string(), text: None,
                    });
                    self.eng.keys.insert(k.to_string(), o);
                }
                "AudioClip" => {
                    let v = pj(raw);
                    let o = new_comp(self, "UnityEngine.AudioClip", Value::Null, CK::AudioClip(v["name"].as_str().unwrap_or("").into()));
                    self.eng.keys.insert(k.to_string(), o);
                }
                "GameObject" => {
                    let v = pj(raw);
                    let d = &v["d"];
                    let o = new_native(
                        self,
                        "UnityEngine.GameObject",
                        Box::new(GoData {
                            name: d["m_Name"].as_str().unwrap_or("").into(),
                            active: jb(&d["m_IsActive"]),
                            port_hidden: jb(&d["m_PortHidden"]),
                            layer: d["m_Layer"].as_i64().unwrap_or(0) as i32,
                            tag: tag_name(d["m_Tag"].as_i64().unwrap_or(0)),
                            comps: vec![],
                            transform: Value::Null,
                            destroyed: false,
                            hier: (0, false),
                            dirty: true,
                            cache: None,
                            in_layout: false,
                        }),
                    );
                    self.eng.keys.insert(k.to_string(), o.clone());
                    self.eng.gos.push(o);
                }
                _ => {}
            }
        }
        // sprites (after textures)
        for (k, raw) in objs.iter() {
            if kind(raw) == "Sprite" {
                let v = pj(raw);
                let d = &v["d"];
                let r = &d["m_Rect"];
                let tex = self.eng.get(&d["m_RD"]["texture"]);
                let b = &d["m_Border"];
                let o = new_comp(
                    self,
                    "UnityEngine.Sprite",
                    Value::Null,
                    CK::Sprite(Box::new(SpriteD {
                        name: d["m_Name"].as_str().unwrap_or("").into(),
                        tex,
                        rect: [jf(&r["x"]), jf(&r["y"]), jf(&r["width"]), jf(&r["height"])],
                        border: [jf(&b["x"]), jf(&b["y"]), jf(&b["z"]), jf(&b["w"])],
                        pivot: jv2(&d["m_Pivot"]),
                        ppu: jf(&d["m_PixelsToUnits"]),
                    })),
                );
                self.eng.keys.insert(k.to_string(), o);
            }
        }
        // 2. components (empty objects first for cross references)
        #[derive(serde::Deserialize)]
        struct ScriptH {
            ns: String,
            cls: String,
            asm: String,
        }
        #[derive(serde::Deserialize, Default)]
        struct HeadD {
            #[serde(default)]
            m_GameObject: Option<String>,
        }
        #[derive(serde::Deserialize)]
        struct Head {
            #[serde(default)]
            script: Option<ScriptH>,
            d: HeadD,
        }
        let mut comp_list: Vec<(String, Value)> = vec![];
        for (k, raw) in objs.iter() {
            let t = kind(raw);
            if !matches!(t, "Transform" | "RectTransform" | "CanvasRenderer" | "Canvas" | "Camera" | "MonoBehaviour") {
                continue;
            }
            let head: Head = match serde_json::from_str(raw.get()) {
                Ok(h) => h,
                Err(_) => continue,
            };
            let goref = match &head.d.m_GameObject {
                Some(s) => self.eng.keys.get(s).cloned().unwrap_or(Value::Null),
                None => Value::Null,
            };
            let o = match t {
                "Transform" | "RectTransform" => new_comp(
                    self,
                    if t == "RectTransform" { "UnityEngine.RectTransform" } else { "UnityEngine.Transform" },
                    goref,
                    CK::Transform(Box::new(Tr { parent: Value::Null, children: vec![], pos: [0.0; 3], scale: [1.0; 3], rot_z: 0.0, amin: [0.5; 2], amax: [0.5; 2], pivot: [0.5; 2], apos: [0.0; 2], size: [0.0; 2], is_rect: t == "RectTransform" })),
                ),
                "CanvasRenderer" => new_comp(self, "UnityEngine.CanvasRenderer", goref, CK::Other("CanvasRenderer".into())),
                "Canvas" => new_comp(self, "UnityEngine.Canvas", goref, CK::Canvas(Box::new(CanvasD { mode: 0, sorting: 0, override_sorting: false, scale_factor: 1.0 }))),
                "Camera" => new_comp(self, "UnityEngine.Camera", goref, CK::Other("Camera".into())),
                _ => {
                    let s = match &head.script {
                        Some(s) => s,
                        None => continue,
                    };
                    let (asm, ns, cls) = (s.asm.as_str(), s.ns.as_str(), s.cls.as_str());
                    if asm == "Assembly-CSharp.dll" {
                        match user_type_id(self, ns, cls) {
                            Some(id) => {
                                let ov = self.construct(id);
                                *ov.x.borrow_mut() = Ext::Native(Box::new(Comp { go: goref, enabled: true, awoken: false, started: false, k: CK::User }));
                                self.eng.user_comps.push(Value::Obj(ov.clone()));
                                self.eng.start_list = None;
                                Value::Obj(ov)
                            }
                            None => new_comp(self, "UnityEngine.MonoBehaviour", goref, CK::Other(format!("?{}", cls))),
                        }
                    } else {
                        let full = format!("{}.{}", ns, cls);
                        let kk = unity_kind(&full);
                        new_comp(self, &full, goref, kk)
                    }
                }
            };
            self.eng.keys.insert(k.to_string(), o.clone());
            comp_list.push((k.to_string(), o));
        }
        // 3. attach components to GameObjects (m_Component order)
        for (k, raw) in objs.iter() {
            if kind(raw) != "GameObject" {
                continue;
            }
            let v = pj(raw);
            let go = self.eng.keys[*k].clone();
            let mut list = vec![];
            let mut tr = Value::Null;
            for c in v["d"]["m_Component"].as_array().cloned().unwrap_or_default() {
                let cv = self.eng.get(&c["component"]);
                if cv.is_null() {
                    continue;
                }
                if with_tr(&cv, |_| ()).is_some() {
                    tr = cv.clone();
                }
                list.push(cv);
            }
            with_go(&go, |g| {
                g.comps = list;
                g.transform = tr;
            });
        }
        // 4. component data (one full parse per object, transient)
        for (k, o) in &comp_list {
            if let Some(raw) = objs.get(k.as_str()) {
                let v = pj(raw);
                self.fill_component(o, &v);
            }
        }
        for (_, o) in &comp_list {
            if with_comp(o, |c| matches!(&c.k, CK::Sel(s) if s.kind == SelKind::Slider || s.kind == SelKind::Scrollbar)).unwrap_or(false) {
                crate::unity_api::slider_visuals(self, o);
            }
        }
        self.eng.canvases = self.eng.gos.iter().filter_map(|go| {
            with_go(go, |g| {
                let sorting = g.comps.iter().filter_map(|component| {
                    with_comp(component, |c| match &c.k {
                        CK::Canvas(canvas) => Some(canvas.sorting),
                        _ => None,
                    }).flatten()
                }).last()?;
                Some((sorting, g.transform.clone()))
            }).flatten()
        }).collect();
        self.eng.canvases.sort_by_key(|(sorting, _)| *sorting);
        // subtrees under a layout
        for go in self.eng.gos.clone() {
            let has = with_go(&go, |g| g.comps.clone()).unwrap_or_default().iter().any(|c| with_comp(c, |k| matches!(k.k, CK::Layout { .. })).unwrap_or(false));
            if !has {
                continue;
            }
            let mut stack = vec![go.clone()];
            while let Some(g) = stack.pop() {
                with_go(&g, |x| x.in_layout = true);
                let tr = with_go(&g, |x| x.transform.clone()).unwrap_or(Value::Null);
                for c in with_tr(&tr, |t| t.children.clone()).unwrap_or_default() {
                    stack.push(go_of(&c));
                }
            }
        }
        self.eng.resources.clear();
        if let Some(resources) = root.resources {
            // A Unity path can refer to both a Texture2D and a Sprite.
            for (path, key) in resources {
                if let Some(asset) = self.eng.keys.get(&key) {
                    self.eng.resources.entry(path.to_ascii_lowercase()).or_default().push(asset.clone());
                }
            }
        } else {
            // Keep name-based lookup for data prepared before the resource index was exported.
            for key in objs.keys().filter(|key| key.starts_with("resources.assets:")) {
                let Some(asset) = self.eng.keys.get(*key) else { continue };
                let name = with_comp(asset, |c| match &c.k {
                    CK::Sprite(s) => Some(s.name.to_ascii_lowercase()),
                    CK::TextAsset { name, .. } => Some(name.to_ascii_lowercase()),
                    CK::AudioClip(name) => Some(format!("sounds/{}", name.to_ascii_lowercase())),
                    _ => None,
                }).flatten();
                if let Some(name) = name {
                    self.eng.resources.entry(name).or_default().push(asset.clone());
                }
            }
        }
        Ok(())
    }

    fn fill_component(&mut self, o: &Value, v: &J) {
        let d = &v["d"];
        let enabled = d.get("m_Enabled").map(jb).unwrap_or(true);
        let t = v["t"].as_str().unwrap_or("");
        let _ = with_comp(o, |c| c.enabled = enabled);
        match t {
            "Transform" | "RectTransform" => {
                let parent = self.eng.get(&d["m_Father"]);
                let children: Vec<Value> = d["m_Children"].as_array().map(|a| a.iter().map(|x| self.eng.get(x)).collect()).unwrap_or_default();
                with_tr(o, |t| {
                    t.parent = parent;
                    t.children = children;
                    t.pos = jv3(&d["m_LocalPosition"]);
                    t.scale = jv3(&d["m_LocalScale"]);
                    let q = &d["m_LocalRotation"];
                    let (z, w) = (jf(&q["z"]), jf(&q["w"]));
                    t.rot_z = 2.0 * z.atan2(w).to_degrees();
                    if t.is_rect {
                        t.amin = jv2(&d["m_AnchorMin"]);
                        t.amax = jv2(&d["m_AnchorMax"]);
                        t.pivot = jv2(&d["m_Pivot"]);
                        t.apos = jv2(&d["m_AnchoredPosition"]);
                        t.size = jv2(&d["m_SizeDelta"]);
                    }
                });
            }
            "Canvas" => {
                with_comp(o, |c| {
                    if let CK::Canvas(cd) = &mut c.k {
                        cd.mode = d["m_RenderMode"].as_i64().unwrap_or(0) as i32;
                        cd.sorting = d["m_SortingOrder"].as_i64().unwrap_or(0) as i32;
                        cd.override_sorting = jb(&d["m_OverrideSorting"]);
                    }
                });
            }
            "MonoBehaviour" => {
                let s = &v["script"];
                let (asm, cls) = (s["asm"].as_str().unwrap_or(""), s["cls"].as_str().unwrap_or(""));
                if asm == "Assembly-CSharp.dll" {
                    if let Value::Obj(ob) = o {
                        let ty = ob.ty;
                        if let Some(m) = d.as_object() {
                            set_fields(self, o, ty, m);
                        }
                    }
                } else {
                    self.fill_ui(o, cls, d);
                }
            }
            _ => {}
        }
    }

    fn fill_ui(&mut self, o: &Value, cls: &str, d: &J) {
        let sprite = self.eng.get(&d["m_Sprite"]);
        let colors = &d["m_Colors"];
        let sel_colors = [jcol(&colors["m_NormalColor"]), jcol(&colors["m_HighlightedColor"]), jcol(&colors["m_PressedColor"]), jcol(&colors["m_SelectedColor"]), jcol(&colors["m_DisabledColor"])];
        match cls {
            "Image" | "RawImage" => {
                let im = ImageD {
                    sprite,
                    color: jcol(&d["m_Color"]),
                    typ: d["m_Type"].as_i64().unwrap_or(0) as i32,
                    fill_center: d.get("m_FillCenter").map(jb).unwrap_or(true),
                    fill_method: d["m_FillMethod"].as_i64().unwrap_or(0) as i32,
                    fill_amount: d.get("m_FillAmount").map(jf).unwrap_or(1.0),
                    fill_origin: d["m_FillOrigin"].as_i64().unwrap_or(0) as i32,
                    clockwise: d.get("m_FillClockwise").map(jb).unwrap_or(true),
                    preserve_aspect: jb(&d["m_PreserveAspect"]),
                    raycast: d.get("m_RaycastTarget").map(jb).unwrap_or(true),
                };
                with_comp(o, |c| c.k = if cls == "Image" { CK::Image(Box::new(im)) } else { CK::RawImage(Box::new(im)) });
            }
            "Text" => {
                let fd = &d["m_FontData"];
                let font = self.eng.get(&fd["m_Font"]);
                let t = TextD {
                    text: d["m_Text"].as_str().unwrap_or("").into(),
                    font,
                    size: fd["m_FontSize"].as_i64().unwrap_or(14) as i32,
                    style: fd["m_FontStyle"].as_i64().unwrap_or(0) as i32,
                    best_fit: jb(&fd["m_BestFit"]),
                    min_size: fd["m_MinSize"].as_i64().unwrap_or(10) as i32,
                    max_size: fd["m_MaxSize"].as_i64().unwrap_or(40) as i32,
                    align: fd["m_Alignment"].as_i64().unwrap_or(0) as i32,
                    rich: fd.get("m_RichText").map(jb).unwrap_or(true),
                    hov: fd["m_HorizontalOverflow"].as_i64().unwrap_or(0) as i32,
                    vov: fd["m_VerticalOverflow"].as_i64().unwrap_or(0) as i32,
                    line_spacing: fd.get("m_LineSpacing").map(jf).unwrap_or(1.0),
                    color: jcol(&d["m_Color"]),
                    raycast: d.get("m_RaycastTarget").map(jb).unwrap_or(true),
                    layout: None,
                };
                with_comp(o, |c| c.k = CK::Text(Box::new(t)));
            }
            "Button" | "Slider" | "Toggle" | "InputField" | "Dropdown" | "Scrollbar" => {
                let kind = match cls {
                    "Button" => SelKind::Button,
                    "Slider" => SelKind::Slider,
                    "Toggle" => SelKind::Toggle,
                    "InputField" => SelKind::InputField,
                    "Dropdown" => SelKind::Dropdown,
                    _ => SelKind::Scrollbar,
                };
                let mut s = default_sel(kind);
                s.interactable = d.get("m_Interactable").map(jb).unwrap_or(true);
                s.transition = d["m_Transition"].as_i64().unwrap_or(1) as i32;
                s.colors = sel_colors;
                s.target_graphic = self.eng.get(&d["m_TargetGraphic"]);
                match kind {
                    SelKind::Button => s.on_click = parse_event(self, &d["m_OnClick"]),
                    SelKind::Slider => {
                        s.min = d.get("m_MinValue").map(jf).unwrap_or(0.0);
                        s.max = d.get("m_MaxValue").map(jf).unwrap_or(1.0);
                        s.whole = jb(&d["m_WholeNumbers"]);
                        s.value = jf(&d["m_Value"]);
                        s.direction = d["m_Direction"].as_i64().unwrap_or(0) as i32;
                        s.fill_rect = self.eng.get(&d["m_FillRect"]);
                        s.handle_rect = self.eng.get(&d["m_HandleRect"]);
                        s.on_change = parse_event(self, &d["m_OnValueChanged"]);
                    }
                    SelKind::Toggle => {
                        s.is_on = jb(&d["m_IsOn"]);
                        s.toggle_graphic = self.eng.get(&d["graphic"]);
                        s.on_change = parse_event(self, &d["onValueChanged"]);
                    }
                    SelKind::InputField => {
                        s.text = d["m_Text"].as_str().unwrap_or("").into();
                        s.text_component = self.eng.get(&d["m_TextComponent"]);
                        s.placeholder = self.eng.get(&d["m_Placeholder"]);
                        s.char_limit = d["m_CharacterLimit"].as_i64().unwrap_or(0) as i32;
                        s.content_type = d["m_ContentType"].as_i64().unwrap_or(0) as i32;
                        s.on_change = parse_event(self, &d["m_OnValueChanged"]);
                        s.on_end = parse_event(self, &d["m_OnEndEdit"]);
                    }
                    SelKind::Dropdown => {
                        s.value = d["m_Value"].as_i64().unwrap_or(0) as f32;
                        s.on_change = parse_event(self, &d["m_OnValueChanged"]);
                        s.caption = self.eng.get(&d["m_CaptionText"]);
                        if let Some(a) = d["m_Options"]["m_Options"].as_array() {
                            let opts: Vec<(String, Value)> = a.iter().map(|x| (x["m_Text"].as_str().unwrap_or("").to_string(), self.eng.get(&x["m_Image"]))).collect();
                            let id = self.type_id(OPTION_DATA);
                            s.options = opts.into_iter().map(|(text, image)| Value::Obj(Obj::new(id, 0, Ext::Native(Box::new(OptD { text, image }))))).collect();
                        }
                    }
                    SelKind::Scrollbar => {
                        s.value = jf(&d["m_Value"]);
                        s.size = jf(&d["m_Size"]);
                        s.direction = d["m_Direction"].as_i64().unwrap_or(0) as i32;
                        s.handle_rect = self.eng.get(&d["m_HandleRect"]);
                        s.on_change = parse_event(self, &d["m_OnValueChanged"]);
                    }
                }
                with_comp(o, |c| c.k = CK::Sel(Box::new(s)));
            }
            "EventTrigger" => {
                let mut list = vec![];
                if let Some(a) = d["m_Delegates"].as_array() {
                    for e in a {
                        let ev = parse_event(self, &e["callback"]);
                        list.push((e["eventID"].as_i64().unwrap_or(0) as i32, ev));
                    }
                }
                with_comp(o, |c| c.k = CK::EventTrigger(list));
            }
            "ScrollRect" => {
                let k = CK::ScrollRect {
                    content: self.eng.get(&d["m_Content"]),
                    viewport: self.eng.get(&d["m_Viewport"]),
                    hscroll: self.eng.get(&d["m_HorizontalScrollbar"]),
                    vscroll: self.eng.get(&d["m_VerticalScrollbar"]),
                    horizontal: jb(&d["m_Horizontal"]),
                    vertical: jb(&d["m_Vertical"]),
                    norm: [0.0, 1.0],
                    sens: d.get("m_ScrollSensitivity").map(jf).unwrap_or(1.0),
                };
                with_comp(o, |c| c.k = k);
                self.eng.scrollrects.push(o.clone());
            }
            "Mask" => {
                with_comp(o, |c| c.k = CK::Mask { show_graphic: jb(&d["m_ShowMaskGraphic"]) });
            }
            "GridLayoutGroup" | "VerticalLayoutGroup" | "HorizontalLayoutGroup" | "ContentSizeFitter" | "LayoutElement" => {
                let mut m = HashMap::new();
                if let Some(obj) = d.as_object() {
                    flatten_nums(obj, "", &mut m);
                }
                with_comp(o, |c| c.k = CK::Layout { kind: cls.to_string(), d: m });
            }
            "Outline" | "Shadow" => {
                let k = CK::Outline { color: jcol(&d["m_EffectColor"]), dist: jv2(&d["m_EffectDistance"]) };
                with_comp(o, |c| c.k = k);
            }
            "CanvasScaler" => {
                let k = CK::CanvasScaler { mode: d["m_UiScaleMode"].as_i64().unwrap_or(0) as i32, scale: d.get("m_ScaleFactor").map(jf).unwrap_or(1.0), reference: jv2(&d["m_ReferenceResolution"]), match_: jf(&d["m_MatchWidthOrHeight"]) };
                with_comp(o, |c| c.k = k);
            }
            _ => {}
        }
    }

}

fn tag_name(i: i64) -> String {
    static CUSTOM: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    let custom = CUSTOM.get_or_init(|| {
        crate::assets::read_string("tags.json")
            .and_then(|t| serde_json::from_str::<J>(&t).ok())
            .and_then(|v| v["tags"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()))
            .unwrap_or_default()
    });
    match i {
        0 => "Untagged".into(),
        1 => "Respawn".into(),
        2 => "Finish".into(),
        3 => "EditorOnly".into(),
        4 => "MainCamera".into(),
        5 => "Player".into(),
        6 => "GameController".into(),
        i if i >= 20000 && ((i - 20000) as usize) < custom.len() => custom[(i - 20000) as usize].clone(),
        _ => format!("Tag{}", i),
    }
}

fn flatten_nums(m: &serde_json::Map<String, J>, prefix: &str, out: &mut HashMap<String, f32>) {
    for (k, v) in m {
        let key = format!("{}{}", prefix, k);
        match v {
            J::Number(n) => {
                out.insert(key, n.as_f64().unwrap_or(0.0) as f32);
            }
            J::Bool(b) => {
                out.insert(key, *b as i32 as f32);
            }
            J::Object(o) => flatten_nums(o, &format!("{}.", key), out),
            _ => {}
        }
    }
}

fn unity_kind(full: &str) -> CK {
    CK::Other(full.to_string())
}
