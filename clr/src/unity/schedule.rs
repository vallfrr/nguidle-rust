use super::*;

impl Vm {
    pub(crate) fn schedule_invoke(&mut self, owner: Value, method: Rc<str>, delay: f64, repeat: f64) {
        let method_id = owner.as_obj().and_then(|obj| self.find_method(obj.ty, &method, 1));
        self.eng.invokes.push(Inv { owner, method, method_id, next: self.eng.time + delay, repeat });
    }

    pub(super) fn run_invokes(&mut self) {
        let now = self.eng.time;
        // a repeating invocation that is late runs once per missed period (Unity's delayed-call queue re-inserts it and keeps
        // popping while it is due), so its rate does not depend on the frame rate; capped to bound a very long frame
        let mut due = vec![];
        for (i, inv) in self.eng.invokes.iter().enumerate() {
            if inv.next <= now {
                due.push(i);
                if inv.repeat > 0.0 {
                    let missed = (((now - inv.next) / inv.repeat) as usize).min(49);
                    due.extend(std::iter::repeat(i).take(missed));
                }
            }
        }
        let mut remove = vec![];
        let profile = self.profile_vita_frame();
        for i in due {
            let (owner, method, method_id, repeat, scheduled) = {
                let v = &mut self.eng.invokes[i];
                let r = (v.owner.clone(), v.method.clone(), v.method_id, v.repeat, v.next);
                if v.repeat > 0.0 {
                    v.next += v.repeat;
                } else {
                    remove.push(i);
                }
                r
            };
            let clock = self.eng.motion.clock;
            if clock.is_some() && repeat > 0.0 {
                self.eng.motion.clock = Some((scheduled, repeat));
            }
            if let (Some(o), Some(m)) = (owner.as_obj(), method_id) {
                let start = profile.then(|| (self.steps, std::time::Instant::now()));
                if let Err(e) = self.invoke(m, [owner.clone()]) {
                    self.report_exc(&method, &owner, &e);
                }
                if let Some((steps, start)) = start {
                    let elapsed = start.elapsed();
                    if elapsed.as_millis() >= 4 {
                        crate::elog!("[slow] frame={} {}::{method} {elapsed:?} instr={}", self.eng.frame, self.types[o.ty as usize].name, self.steps - steps);
                    }
                }
            }
            self.eng.motion.clock = clock;
        }
        for i in remove.into_iter().rev() {
            self.eng.invokes.remove(i);
        }
    }

    pub fn start_coroutine(&mut self, owner: &Value, e: Value) -> Value {
        self.eng.coroutines.push(Co { owner: owner.clone(), stack: vec![e], wait: Wait::Frame, dead: false });
        let id = self.type_id("UnityEngine.Coroutine");
        // first immediate advance (like Unity)
        let idx = self.eng.coroutines.len() - 1;
        self.advance_co(idx);
        Value::Obj(Obj::new(id, 0, Ext::None))
    }

    pub(super) fn run_coroutines(&mut self) {
        let n = self.eng.coroutines.len();
        for i in 0..n {
            if i >= self.eng.coroutines.len() {
                break;
            }
            if self.eng.coroutines[i].dead {
                continue;
            }
            let ready = match self.eng.coroutines[i].wait {
                Wait::Frame => true,
                Wait::Until(t) => self.eng.time >= t,
                Wait::Done => true,
            };
            if ready {
                self.advance_co(i);
            }
        }
        self.eng.coroutines.retain(|c| !c.dead);
    }

    fn advance_co(&mut self, i: usize) {
        loop {
            let top = match self.eng.coroutines[i].stack.last() {
                Some(t) => t.clone(),
                None => {
                    self.eng.coroutines[i].dead = true;
                    return;
                }
            };
            let ty = match &top {
                Value::Obj(o) => o.ty,
                _ => {
                    self.eng.coroutines[i].dead = true;
                    return;
                }
            };
            let mn = self.find_virtual_by_name(ty, "MoveNext", 1);
            let cur = self.find_virtual_by_name(ty, "get_Current", 1);
            let (mn, cur) = match (mn, cur) {
                (Some(a), Some(b)) => (a, b),
                _ => {
                    self.eng.coroutines[i].dead = true;
                    return;
                }
            };
            let r = self.invoke(mn, vec![top.clone()]);
            match r {
                Err(e) => {
                    let owner = self.eng.coroutines[i].owner.clone();
                    self.report_exc("coroutine", &owner, &e);
                    self.eng.coroutines[i].dead = true;
                    return;
                }
                Ok(v) => {
                    if !v.as_bool() {
                        self.eng.coroutines[i].stack.pop();
                        if self.eng.coroutines[i].stack.is_empty() {
                            self.eng.coroutines[i].dead = true;
                            return;
                        }
                        continue;
                    }
                    let c = self.invoke(cur, vec![top.clone()]).unwrap_or(Value::Null);
                    // what must be waited for?
                    match &c {
                        Value::Obj(o) => {
                            let secs = if let Ext::Native(b) = &*o.x.borrow() {
                                b.downcast_ref::<Comp>().and_then(|k| if let CK::Wait(s) = k.k { Some(s) } else { None })
                            } else {
                                None
                            };
                            if let Some(s) = secs {
                                self.eng.coroutines[i].wait = Wait::Until(self.eng.time + s as f64);
                                return;
                            }
                            // nested enumerator
                            if self.find_virtual_by_name(o.ty, "MoveNext", 1).is_some() {
                                self.eng.coroutines[i].stack.push(c.clone());
                                continue;
                            }
                            self.eng.coroutines[i].wait = Wait::Frame;
                            return;
                        }
                        _ => {
                            self.eng.coroutines[i].wait = Wait::Frame;
                            return;
                        }
                    }
                }
            }
        }
    }
}
