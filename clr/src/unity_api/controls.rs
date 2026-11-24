use super::*;

pub(super) fn lookup(key: &str) -> Option<NativeFn> {
    let function: NativeFn = match key {
        // ------------------------------------------------------- Input
        "UnityEngine.Input::get_mousePosition" => |vm, _a| {
            let (x, y) = vm.eng.mouse;
            Ok(vec3(vm, [x, y, 0.0]))
        },
        "UnityEngine.Input::GetMouseButton" => |vm, a| Ok(Value::bool(vm.eng.mouse_down && a[0].as_i32() == vm.eng.mouse_button)),
        "UnityEngine.Input::GetMouseButtonDown" => |vm, a| Ok(Value::bool(vm.eng.mouse_down_edge && a[0].as_i32() == vm.eng.mouse_button)),
        "UnityEngine.Input::GetMouseButtonUp" => |vm, a| Ok(Value::bool(vm.eng.mouse_up_edge && a[0].as_i32() == vm.eng.mouse_button)),
        "UnityEngine.Input::GetKey" => |vm, a| {
            let k = key_arg(vm, &a[0]);
            Ok(Value::bool(vm.eng.keys_down.contains(&k)))
        },
        "UnityEngine.Input::GetKeyDown" => |vm, a| {
            let k = key_arg(vm, &a[0]);
            Ok(Value::bool(vm.eng.keys_edge.contains(&k)))
        },
        // ------------------------------------------------------- PlayerPrefs / Resources / TextAsset
        "UnityEngine.PlayerPrefs::GetInt" => |vm, a| {
            let k = a[0].as_str().unwrap_or("").to_string();
            Ok(match vm.eng.prefs.get(&k) {
                Some(Value::I32(i)) => Value::I32(*i),
                _ => Value::I32(a.get(1).map(|x| x.as_i32()).unwrap_or(0)),
            })
        },
        "UnityEngine.PlayerPrefs::GetString" => |vm, a| {
            let k = a[0].as_str().unwrap_or("").to_string();
            Ok(match vm.eng.prefs.get(&k) {
                Some(Value::Str(s)) => Value::Str(s.clone()),
                _ => a.get(1).cloned().unwrap_or_else(|| Value::str("")),
            })
        },
        "UnityEngine.PlayerPrefs::SetInt" | "UnityEngine.PlayerPrefs::SetString" => |vm, a| {
            let k = a[0].as_str().unwrap_or("").to_string();
            vm.eng.prefs.insert(k, a[1].clone());
            save_prefs(vm);
            Ok(Value::Null)
        },
        "UnityEngine.Resources::Load" => |vm, a| {
            let path = a[0].as_str().unwrap_or("").to_ascii_lowercase();
            let ty = vm.targs.as_deref().and_then(|args| args.first()).map(|t| crate::lower::ty_name(t, vm));
            let asset = vm.eng.resources.get(&path).and_then(|assets| {
                assets.iter().find(|asset| match ty.as_deref() {
                    None | Some("UnityEngine.Object" | "System.Object") => true,
                    Some(ty) => matches!(asset, Value::Obj(o) if {
                        let name = vm.types[o.ty as usize].name.as_str();
                        name == ty || (name == "UnityEngine.Texture2D" && ty == "UnityEngine.Texture")
                    }),
                })
            });
            Ok(asset.cloned().unwrap_or(Value::Null))
        },
        "UnityEngine.TextAsset::get_text" => |vm, a| {
            let v = rv(vm, &a[0]);
            Ok(with_comp(&v, |c| {
                if let CK::TextAsset { key, text, .. } = &mut c.k {
                    if let Some(text) = text {
                        return Value::Str(text.clone());
                    }
                    if let Some(bytes) = crate::assets::read(&format!("text/{}.bin", key.replace(':', "_"))) {
                        let loaded = Rc::<str>::from(String::from_utf8_lossy(&bytes).as_ref());
                        *text = Some(loaded.clone());
                        return Value::Str(loaded);
                    }
                }
                Value::str("")
            }).unwrap_or_else(|| Value::str("")))
        },
        // ------------------------------------------------------- UI
        "UnityEngine.UI.Text::get_text" => |vm, a| {
            let v = rv(vm, &a[0]);
            Ok(with_comp(&v, |c| if let CK::Text(t) = &c.k { Value::str(&t.text) } else { Value::Null }).unwrap_or(Value::Null))
        },
        "UnityEngine.UI.Text::set_text" => |vm, a| {
            let v = rv(vm, &a[0]);
            let s = to_string(vm, &a[1])?;
            set_text_comp(&v, &s);
            Ok(Value::Null)
        },
        "UnityEngine.UI.Text::set_fontSize" => |vm, a| {
            let v = rv(vm, &a[0]);
            with_comp(&v, |c| if let CK::Text(t) = &mut c.k { t.size = a[1].as_i32() });
            mark_dirty(&v);
            bump_layout(&v);
            Ok(Value::Null)
        },
        "UnityEngine.UI.Text::set_alignment" => |vm, a| {
            let v = rv(vm, &a[0]);
            with_comp(&v, |c| if let CK::Text(t) = &mut c.k { t.align = a[1].as_i32() });
            mark_dirty(&v);
            bump_layout(&v);
            Ok(Value::Null)
        },
        "UnityEngine.UI.Graphic::set_color" => |vm, a| {
            let v = rv(vm, &a[0]);
            let c = to_col(&rv(vm, &a[1]));
            let (ch, old_text) = with_comp(&v, |k| match &mut k.k {
                CK::Image(i) | CK::RawImage(i) => {
                    let old = std::mem::replace(&mut i.color, c);
                    if i.raycast && (old[3] > 0.0) != (c[3] > 0.0) {
                        invalidate_input();
                    }
                    (old != c, None)
                }
                CK::Text(t) => {
                    let old = std::mem::replace(&mut t.color, c);
                    (old != c, Some(old))
                }
                _ => (false, None),
            }).unwrap_or_default();
            if ch {
                if let Some(old) = old_text { crate::motion::text_color_changed(vm, &v, old, c); }
                mark_dirty(&v);
            }
            Ok(Value::Null)
        },
        "UnityEngine.UI.Graphic::get_color" => |vm, a| {
            let v = rv(vm, &a[0]);
            let c = with_comp(&v, |k| match &k.k {
                CK::Image(i) | CK::RawImage(i) => i.color,
                CK::Text(t) => t.color,
                _ => [1.0; 4],
            })
            .unwrap_or([1.0; 4]);
            Ok(col(vm, c))
        },
        "UnityEngine.UI.Image::set_sprite" => |vm, a| {
            let v = rv(vm, &a[0]);
            let old = with_comp(&v, |k| if let CK::Image(i) = &mut k.k { Some(std::mem::replace(&mut i.sprite, a[1].clone())) } else { None }).flatten();
            if let Some(old) = old.filter(|old| !same(old, &a[1])) {
                mark_dirty(&v);
                // The layout only reads a sprite's size (rect and pixels per unit): a swap between same-sized sprites
                // (a blinking highlight, an item icon) cannot move anything, so it must not relayout the whole scene.
                let size = |sp: &Value| with_comp(sp, |k| if let CK::Sprite(s) = &k.k { Some((s.rect[2], s.rect[3], s.ppu)) } else { None }).flatten();
                if size(&old) != size(&a[1]) {
                    bump_layout(&v);
                }
            }
            Ok(Value::Null)
        },
        "UnityEngine.UI.Image::get_sprite" => |vm, a| {
            let v = rv(vm, &a[0]);
            Ok(with_comp(&v, |k| if let CK::Image(i) = &k.k { i.sprite.clone() } else { Value::Null }).unwrap_or(Value::Null))
        },
        "UnityEngine.UI.RawImage::set_texture" => stub0!(),
        "UnityEngine.UI.Selectable::set_interactable" => |vm, a| {
            let v = rv(vm, &a[0]);
            if sel(&v, |s| std::mem::replace(&mut s.interactable, a[1].as_bool()) != a[1].as_bool()) == Some(true) {
                mark_sel_dirty(&v);
                invalidate_input();
            }
            Ok(Value::Null)
        },
        "UnityEngine.UI.Selectable::get_interactable" | "UnityEngine.UI.Selectable::IsInteractable" => |vm, a| {
            let v = rv(vm, &a[0]);
            Ok(Value::bool(sel(&v, |s| s.interactable).unwrap_or(true)))
        },
        "UnityEngine.UI.Selectable::get_image" => |vm, a| {
            let v = rv(vm, &a[0]);
            Ok(sel(&v, |s| s.target_graphic.clone()).unwrap_or(Value::Null))
        },
        "UnityEngine.UI.Button::get_onClick" => |vm, a| {
            let v = rv(vm, &a[0]);
            Ok(sel(&v, |s| s.on_click.clone()).unwrap_or(Value::Null))
        },
        "UnityEngine.Events.UnityEvent::AddListener" => |vm, a| {
            let ev = rv(vm, &a[0]);
            let d = if let Value::Obj(o) = &a[1] {
                if let Ext::Delegate(l) = &*o.x.borrow() { l.clone() } else { vec![] }
            } else {
                vec![]
            };
            with_comp(&ev, |c| if let CK::Event(e) = &mut c.k { e.runtime.extend(d) });
            Ok(Value::Null)
        },
        "UnityEngine.Events.UnityEventBase::RemoveAllListeners" => |vm, a| {
            let ev = rv(vm, &a[0]);
            with_comp(&ev, |c| if let CK::Event(e) = &mut c.k { e.runtime.clear() });
            Ok(Value::Null)
        },
        "UnityEngine.Events.UnityEvent::Invoke" => |vm, a| {
            let ev = rv(vm, &a[0]);
            vm.invoke_event(&ev, vec![]);
            Ok(Value::Null)
        },
        "UnityEngine.UI.Slider::get_value" | "UnityEngine.UI.Toggle::get_isOn" | "UnityEngine.UI.Scrollbar::get_value" | "UnityEngine.UI.Dropdown::get_value" => |vm, a| {
            let v = rv(vm, &a[0]);
            Ok(sel(&v, |s| match s.kind {
                SelKind::Toggle => Value::bool(s.is_on),
                SelKind::Dropdown => Value::I32(s.value as i32),
                _ => Value::F32(s.value),
            })
            .unwrap_or(Value::Null))
        },
        "UnityEngine.UI.Dropdown::set_value" => |vm, a| {
            let comp = rv(vm, &a[0]);
            set_dropdown_value(vm, &comp, a[1].as_i32());
            Ok(Value::Null)
        },
        "UnityEngine.UI.Slider::set_value" | "UnityEngine.UI.Scrollbar::set_value" => |vm, a| {
            let v = rv(vm, &a[0]);
            let nv = a[1].as_f64() as f32;
            let (changed, on_change, kind) = sel(&v, |s| {
                let nv = if s.whole { nv.round() } else { nv.clamp(s.min.min(s.max), s.max.max(s.min)) };
                let ch = (s.value - nv).abs() > 1e-7;
                s.value = nv;
                (ch, s.on_change.clone(), s.kind)
            })
            .unwrap_or((false, Value::Null, SelKind::Slider));
            if changed {
                slider_visuals(vm, &v);
            }
            if kind == SelKind::Scrollbar && changed {
                vm.scrollbar_moved(&v);
            }
            if changed && !on_change.is_null() {
                let val = sel(&v, |s| s.value).unwrap_or(0.0);
                vm.invoke_event(&on_change, vec![Value::F32(val)]);
            }
            Ok(Value::Null)
        },
        "UnityEngine.UI.InputField::get_text" => |vm, a| {
            let v = rv(vm, &a[0]);
            Ok(sel(&v, |s| Value::str(&s.text)).unwrap_or(Value::Null))
        },
        "UnityEngine.UI.InputField::set_text" => |vm, a| {
            let v = rv(vm, &a[0]);
            let s = to_string(vm, &a[1])?;
            let tc = sel(&v, |x| {
                x.text = s.clone();
                x.text_component.clone()
            })
            .unwrap_or(Value::Null);
            set_text_comp(&tc, &s);
            Ok(Value::Null)
        },
        "UnityEngine.UI.InputField::get_isFocused" => |vm, a| {
            let v = rv(vm, &a[0]);
            Ok(Value::bool(sel(&v, |s| s.focused).unwrap_or(false)))
        },
        "UnityEngine.UI.Dropdown::ClearOptions" => |vm, a| {
            let v = rv(vm, &a[0]);
            vm.hide_dropdown(&v);
            sel(&v, |s| s.options.clear());
            Ok(Value::Null)
        },
        "UnityEngine.UI.Dropdown::Hide" => |vm, a| {
            let comp = rv(vm, &a[0]);
            vm.hide_dropdown(&comp);
            Ok(Value::Null)
        },
        "UnityEngine.UI.Dropdown::RefreshShownValue" => |vm, a| {
            // the caption shows the text of the selected option
            let v = rv(vm, &a[0]);
            let (cap, opt) = sel(&v, |s| (s.caption.clone(), s.options.get(s.value as usize).cloned())).unwrap_or((Value::Null, None));
            if let Some(o) = opt {
                if let Some(t) = opt_data(&o, |d| d.text.clone()) {
                    set_text_comp(&cap, &t);
                }
            }
            Ok(Value::Null)
        },
        "UnityEngine.UI.Dropdown::AddOptions" => |vm, a| {
            // List<string> or List<OptionData>
            let v = rv(vm, &a[0]);
            let items = iter_values(vm, &a[1])?;
            let id = vm.type_id(OPTION_DATA);
            let mut opts = vec![];
            for it in items {
                if opt_data(&it, |_| ()).is_some() {
                    opts.push(it);
                } else {
                    let text = to_string(vm, &it)?;
                    opts.push(Value::Obj(Obj::new(id, 0, Ext::Native(Box::new(OptD { text, image: Value::Null })))));
                }
            }
            sel(&v, |s| s.options.extend(opts));
            Ok(Value::Null)
        },
        "UnityEngine.UI.Dropdown::get_options" => |vm, a| {
            let v = rv(vm, &a[0]);
            let items = sel(&v, |s| s.options.clone()).unwrap_or_default();
            Ok(crate::bcl_core::new_list(vm, items))
        },
        "UnityEngine.UI.Dropdown::get_captionText" => |vm, a| {
            let v = rv(vm, &a[0]);
            Ok(sel(&v, |s| s.caption.clone()).unwrap_or(Value::Null))
        },
        "UnityEngine.UI.Dropdown/OptionData::get_text" => |vm, a| {
            let v = rv(vm, &a[0]);
            Ok(Value::str(&opt_data(&v, |d| d.text.clone()).unwrap_or_default()))
        },
        "UnityEngine.UI.Dropdown/OptionData::set_text" => |vm, a| {
            let v = rv(vm, &a[0]);
            let t = to_string(vm, &a[1])?;
            opt_data(&v, |d| d.text = t);
            Ok(Value::Null)
        },
        "UnityEngine.UI.Dropdown/OptionData::get_image" => |vm, a| {
            let v = rv(vm, &a[0]);
            Ok(opt_data(&v, |d| d.image.clone()).unwrap_or(Value::Null))
        },
        "UnityEngine.UI.Dropdown/OptionData::set_image" => |vm, a| {
            let v = rv(vm, &a[0]);
            let img = a[1].clone();
            opt_data(&v, |d| d.image = img);
            Ok(Value::Null)
        },
        "UnityEngine.EventSystems.PointerEventData::get_button" => |vm, a| {
            let v = rv(vm, &a[0]);
            Ok(Value::I32(crate::input::with_ped(&v, |p| p.button).unwrap_or(0)))
        },
        "UnityEngine.EventSystems.PointerEventData::get_pointerId" => |_vm, _a| Ok(Value::I32(-1)),
        "UnityEngine.EventSystems.PointerEventData::get_clickCount" => |vm, a| {
            let v = rv(vm, &a[0]);
            Ok(Value::I32(crate::input::with_ped(&v, |p| p.click_count).unwrap_or(1)))
        },
        "UnityEngine.EventSystems.PointerEventData::get_pointerPress" => |vm, a| {
            let v = rv(vm, &a[0]);
            Ok(crate::input::with_ped(&v, |p| p.press.clone()).unwrap_or(Value::Null))
        },
        "UnityEngine.EventSystems.PointerEventData::get_position" => |vm, a| {
            let v = rv(vm, &a[0]);
            let p = crate::input::with_ped(&v, |p| p.pos).unwrap_or([0.0; 2]);
            Ok(vec2(vm, p))
        },
        _ => return None,
    };
    Some(function)
}
