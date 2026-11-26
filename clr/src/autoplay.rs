//! Scripted player for benchmarks (`docs/VITA-60.md`): replays a scenario through the same inputs the console sends
//! (`ngu_pad`, `ngu_frame` touch, `ngu_scroll`) so a real play session can be repeated on the Vita and on PC.
//!
//! Scenario file, one event per line, fired when the game has run `tick` simulation steps (30 per second of game time,
//! independent of the frame rate, so a slow frame never reorders the session):
//! ```text
//! # comment
//! <tick> pad <chars>        D-pad/buttons one per frame: u d l r (focus), a (confirm), b (back), p n (previous/next menu)
//! <tick> touch <x> <y>      tap at screen pixels (top-left origin)
//! <tick> scroll <dx> <dy>   right stick
//! <tick> menu <n>           MenuSwapper.swapMenu(n)
//! <tick> click <text>       virtual click on the visible control whose label contains <text>
//! <tick> mark <name>        names the phase that follows (shows in the trace and the analysis)
//! <tick> check <name>       digest of the serialized save (determinism check, see docs)
//! <tick> ui                screen as text in the log (to write scenarios)
//! <tick> end                scenario finished
//! ```
use crate::Vm;
use std::collections::VecDeque;

#[derive(Clone, Debug, PartialEq)]
pub enum Act {
    Pad(char),
    Touch(f32, f32),
    Scroll(f32, f32),
    Menu(i32),
    Click(String),
    Mark(String),
    Check(String),
    /// the host saves a screenshot (Vita benchmark build: bench/shotN.tga)
    Shot,
    /// debug aid: writes the screen as text to the log
    Ui,
    End,
}

#[derive(Default)]
pub struct Autoplay {
    events: Vec<(u64, Vec<Act>)>,
    next: usize,
    queue: VecDeque<Act>,
    /// touch held for this many more frames: (x, y)
    pub hold: Option<(f32, f32, u8)>,
    /// number of the last executed action (1-based, 0 = none): the trace links frame spikes to it
    pub action_id: u32,
    pub marks: Vec<String>,
    pub digests: Vec<(String, u64)>,
    pub done: bool,
    /// `shot` events executed so far (the host takes a screenshot each time this grows)
    pub shots: u32,
}

pub fn parse(text: &str) -> Result<Vec<(u64, Vec<Act>)>, String> {
    let mut out: Vec<(u64, Vec<Act>)> = vec![];
    for (n, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let err = |m: &str| format!("scenario line {}: {m}", n + 1);
        let mut w = line.splitn(3, char::is_whitespace);
        let tick: u64 = w.next().and_then(|t| t.parse().ok()).ok_or_else(|| err("tick expected"))?;
        let verb = w.next().ok_or_else(|| err("action expected"))?;
        let rest = w.next().unwrap_or("").trim();
        let num = |s: &str| s.parse::<f32>().map_err(|_| err("bad number"));
        let acts: Vec<Act> = match verb {
            "pad" => rest.chars().filter(|c| !c.is_whitespace()).map(Act::Pad).collect(),
            "touch" | "scroll" => {
                let (a, b) = rest.split_once(char::is_whitespace).ok_or_else(|| err("two numbers expected"))?;
                let (a, b) = (num(a.trim())?, num(b.trim())?);
                vec![if verb == "touch" { Act::Touch(a, b) } else { Act::Scroll(a, b) }]
            }
            "menu" => vec![Act::Menu(rest.parse().map_err(|_| err("bad menu"))?)],
            "click" => vec![Act::Click(rest.to_string())],
            "mark" => vec![Act::Mark(rest.to_string())],
            "check" => vec![Act::Check(rest.to_string())],
            "shot" => vec![Act::Shot],
            "ui" => vec![Act::Ui],
            "end" => vec![Act::End],
            _ => return Err(err(&format!("unknown action {verb}"))),
        };
        match out.last_mut() {
            Some((t, v)) if *t == tick => v.extend(acts),
            Some((t, _)) if *t > tick => return Err(err("ticks must not decrease")),
            _ => out.push((tick, acts)),
        }
    }
    Ok(out)
}

impl Autoplay {
    pub fn load(path: &str) -> Result<Autoplay, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
        Ok(Autoplay { events: parse(&text)?, ..Default::default() })
    }

    /// Next action to run this frame (at most one per frame), given the number of simulation steps done so far.
    pub fn poll(&mut self, ticks: u64) -> Option<Act> {
        while self.next < self.events.len() && self.events[self.next].0 <= ticks {
            let acts = std::mem::take(&mut self.events[self.next].1);
            self.queue.extend(acts);
            self.next += 1;
        }
        let a = self.queue.pop_front()?;
        self.action_id += 1;
        if a == Act::End {
            self.done = true;
        }
        Some(a)
    }
}

/// FNV-1a: small and stable, enough to tell two saves apart.
pub fn digest(s: &str) -> u64 {
    s.bytes().fold(0xcbf29ce484222325, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3))
}

/// Serialized save (the base64 text the game writes) of the current state.
pub fn save_text(vm: &mut Vm) -> Option<String> {
    for c in vm.eng.user_comps.clone() {
        let crate::value::Value::Obj(o) = &c else { continue };
        if vm.types[o.ty as usize].name == "ImportExport" {
            let m = vm.find_method(o.ty, "getBase64Data", 1)?;
            let v = vm.invoke(m, vec![c.clone()]).ok()?;
            return crate::bcl_core::to_string(vm, &v).ok();
        }
    }
    None
}

pub fn swap_menu(vm: &mut Vm, n: i32) {
    for c in vm.eng.user_comps.clone() {
        let crate::value::Value::Obj(o) = &c else { continue };
        if vm.types[o.ty as usize].name == "MenuSwapper" {
            if let Some(m) = vm.find_method(o.ty, "swapMenu", 2) {
                let _ = vm.invoke(m, vec![c.clone(), crate::value::Value::I32(n)]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scenario_fires_by_tick_one_action_per_frame() {
        let ev = parse("# demo\n0 mark start\n10 pad dd a\n10 touch 480 300\n40 end\n").unwrap();
        let mut p = Autoplay { events: ev, ..Default::default() };
        assert_eq!(p.poll(0), Some(Act::Mark("start".into())));
        assert_eq!(p.poll(5), None);
        let got: Vec<Act> = (0..5).filter_map(|_| p.poll(10)).collect();
        assert_eq!(got, vec![Act::Pad('d'), Act::Pad('d'), Act::Pad('a'), Act::Touch(480.0, 300.0)]);
        assert!(!p.done);
        assert_eq!(p.poll(41), Some(Act::End));
        assert!(p.done && p.action_id == 6);
        assert!(parse("5 pad a\n3 pad a").is_err());
    }
}

/// PC runner (`clr <dll> autoplay <assets dir> <scenario> <trace.csv> [max frames]`): replays a scenario headless through the
/// console's C API at a virtual 60 Hz and writes one CSV row per frame (same columns as the Vita trace where they exist).
/// Needs `NGU_SAVE_DIR` (disposable save folder); `NGU_FIXED_NOW` makes the save digests repeatable.
pub fn run_pc(args: &[String]) -> Result<(), String> {
    use crate::capi::*;
    use std::io::Write;
    let [assets, scenario, csv, rest @ ..] = args else { return Err("usage: clr <dll> autoplay <assets> <scenario> <trace.csv> [max frames]".into()) };
    let max: u64 = rest.first().and_then(|m| m.parse().ok()).unwrap_or(60 * 60 * 40);
    let c = |s: &str| std::ffi::CString::new(s).unwrap();
    if std::env::var_os("NGU_SAVE_DIR").is_none() {
        return Err("set NGU_SAVE_DIR to a disposable folder".into());
    }
    if ngu_init(c(assets).as_ptr(), 960, 544) != 0 || ngu_autoplay_load(c(scenario).as_ptr()) != 0 {
        return Err("init or scenario failed (see the log in the assets folder)".into());
    }
    let mut out = std::io::BufWriter::new(std::fs::File::create(csv).map_err(|e| e.to_string())?);
    writeln!(out, "frame,tick,steps,instr,menu,action,marks,logic_us,emit_us").ok();
    let mut frame = 0u64;
    let mut info = [0u32; 7];
    while frame < max {
        let t = std::time::Instant::now();
        ngu_frame(1.0 / 60.0, -1.0, -1.0, 0);
        let logic = t.elapsed().as_micros();
        let t = std::time::Instant::now();
        ngu_emit();
        let emit = t.elapsed().as_micros();
        ngu_frame_info(info.as_mut_ptr());
        writeln!(out, "{frame},{},{},{},{},{},{},{logic},{emit}", info[0], info[1], info[2], info[3] as i32, info[4], info[5]).ok();
        frame += 1;
        if info[6] != 0 {
            break;
        }
    }
    println!("[autoplay] {frame} frames, {} ticks", info[0]);
    if std::env::var_os("CLR_PROF").is_some() {
        print!("{}", profile_report(std::env::var("NGU_PROF_TOP").ok().and_then(|v| v.parse().ok()).unwrap_or(25)));
    }
    Ok(())
}
