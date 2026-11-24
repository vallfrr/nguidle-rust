use super::*;

pub(super) struct WishTickFields {
    pub(super) caller: u32,
    pub(super) character: (u32, usize),
    pub(super) wishes: (u32, usize),
    pub(super) entries: (u32, usize),
    pub(super) wish_type: u32,
    pub(super) resources: [usize; 3],
}

impl WishTickFields {
    pub(super) fn resolve(vm: &Vm) -> Option<Self> {
        let field = |name, field| field_ref(vm, name, field);
        let character = field("WishesController", "character")?;
        let (wish_type, energy) = field("Wish", "energy")?;
        Some(Self {
            caller: vm.find_method(character.0, "updateAllWishes", 1)?,
            character,
            wishes: field("Character", "wishes")?,
            entries: field("Wishes", "wishes")?,
            wish_type,
            resources: [energy, field("Wish", "magic")?.1, field("Wish", "res3")?.1],
        })
    }

    pub(super) fn entries(&self, controller: &Value) -> Option<Rc<Obj>> {
        let character = read_field(controller, self.character)?;
        let wishes = read_field(&character, self.wishes)?;
        let Value::Obj(entries) = read_field(&wishes, self.entries)? else { return None };
        Some(entries)
    }

    pub(super) fn unallocated(&self, wish: &Value) -> bool {
        let Value::Obj(wish) = wish else { return false };
        if wish.ty != self.wish_type { return false; }
        wish.f.try_borrow().is_ok_and(|fields| {
            self.resources.iter().any(|&slot| matches!(fields.get(slot), Some(Value::I64(0))))
        })
    }

    pub(super) fn no_progress(&self, stack: &[Value], caller: Option<&u32>) -> bool {
        if caller != Some(&self.caller) { return false; }
        let [controller, Value::I32(index), ..] = stack else { return false };
        let Some(entries) = self.entries(controller) else { return false };
        let Ok(entries) = entries.x.try_borrow() else { return false };
        let Ext::List(entries) = &*entries else { return false };
        entries.get(*index as usize).is_some_and(|wish| self.unallocated(wish))
    }

    pub(super) fn all_idle(&self, controller: &Value) -> Option<bool> {
        let entries = self.entries(controller)?;
        let entries = entries.x.try_borrow().ok()?;
        let Ext::List(entries) = &*entries else { return None };
        Some(entries.iter().all(|wish| self.unallocated(wish)))
    }
}

/// Field slots for the native `InventoryController.equipSpecBonus` / `specBonus` (guards 20, 21).
#[derive(Clone, Copy)]
pub(super) struct EquipSpecFields {
    pub(super) character: (u32, usize),
    pub(super) challenges: (u32, usize),
    pub(super) no_equipment: (u32, usize),
    pub(super) in_challenge: (u32, usize),
    pub(super) inventory: (u32, usize),
    pub(super) disabled: (u32, usize),
    pub(super) spec_type: [(u32, usize); 3],
    pub(super) spec_cur: [(u32, usize); 3],
    pub(super) boss_required: (u32, usize),
    /// Inventory: weapon, head, chest, legs, boots, weapon2, accs
    pub(super) parts: [(u32, usize); 7],
    pub(super) weapon2_factor: u32,
}

impl EquipSpecFields {
    pub(super) fn resolve(vm: &Vm) -> Option<Self> {
        let f = |t, n| field_ref(vm, t, n);
        Some(Self {
            character: f("InventoryController", "character")?,
            challenges: f("Character", "challenges")?,
            no_equipment: f("Challenges", "noEquipmentChallenge")?,
            in_challenge: f("Challenge", "inChallenge")?,
            inventory: f("Character", "inventory")?,
            disabled: f("Inventory", "disabled")?,
            spec_type: [f("Equipment", "spec1Type")?, f("Equipment", "spec2Type")?, f("Equipment", "spec3Type")?],
            spec_cur: [f("Equipment", "spec1Cur")?, f("Equipment", "spec2Cur")?, f("Equipment", "spec3Cur")?],
            boss_required: f("Equipment", "bossRequired")?,
            parts: ["weapon", "head", "chest", "legs", "boots", "weapon2", "accs"].map(|n| f("Inventory", n)).into_iter().collect::<Option<Vec<_>>>()?.try_into().ok()?,
            weapon2_factor: vm.find_method(f("InventoryController", "character")?.0, "weapon2Factor", 1)?,
        })
    }
}

pub(super) struct TitanChecks {
    pub(super) scope: u32,
    pub(super) character: Rc<Obj>,
    pub(super) enabled: bool,
    pub(super) stats: [Option<f32>; 3],
}

impl Vm {
    pub(super) fn native_equip_spec_bonus(&self, base: usize) -> Option<Value> {
        let f = self.equip_spec_fields.as_ref()?;
        let (this, ty, equip) = (self.slots.get(base)?, self.slots.get(base + 1)?, self.slots.get(base + 2)?);
        let Value::I32(ty) = ty else { return None };
        let character = read_field(this, f.character)?;
        self.equip_spec_value(f, &character, *ty, equip).map(Value::F32)
    }

    /// `InventoryController.specBonus(type)`: sum over the equipped items in the IL's order (weapon, head, chest, legs, boots,
    /// weapon2 x weapon2Factor, accessories). The factor is only evaluated when weapon2 contributes (a zero term stays zero).
    pub(super) fn native_spec_bonus(&mut self, base: usize) -> Option<Value> {
        let f = *self.equip_spec_fields.as_ref()?;
        let [boss, settings, difficulty] = self.effective_boss_fields?;
        let (this, ty) = (self.slots.get(base)?.clone(), self.slots.get(base + 1)?.clone());
        let Value::I32(ty) = ty else { return None };
        let character = read_field(&this, f.character)?;
        let inv = read_field(&character, f.inventory)?;
        // The challenge, the disabled flag and the boss number are the same for every item of one call: read them once.
        let no_equipment = read_field(&read_field(&character, f.challenges)?, f.no_equipment)?;
        let Value::I32(in_challenge) = read_field(&no_equipment, f.in_challenge)? else { return None };
        let Value::I32(disabled) = read_field(&inv, f.disabled)? else { return None };
        if in_challenge != 0 || disabled != 0 {
            // every item contributes 0 (weapon2Factor() is a pure formula: 0 x factor stays 0)
            return Some(Value::F32(0.0));
        }
        let Value::I32(boss_id) = read_field(&character, boss)? else { return None };
        let Value::I32(difficulty) = read_field(&read_field(&character, settings)?, difficulty)? else { return None };
        let boss_id = boss_id.wrapping_add(301 * (i32::from(difficulty >= 1) + i32::from(difficulty >= 2)));
        let part = |i: usize| read_field(&inv, f.parts[i]);
        let mut num = Self::item_spec(&f, boss_id, ty, &part(0)?)?;
        for i in 1..5 {
            num += Self::item_spec(&f, boss_id, ty, &part(i)?)?;
        }
        let w2 = Self::item_spec(&f, boss_id, ty, &part(5)?)?;
        let mut term = 0f32;
        if w2 != 0.0 {
            let m = f.weapon2_factor;
            let Value::F32(factor) = self.invoke(m, vec![this]).ok()? else { return None };
            term = w2 * factor;
        }
        num += term;
        let Value::Obj(accs) = part(6)? else { return None };
        let Ext::List(accs) = &*accs.x.try_borrow().ok()? else { return None };
        for acc in accs.iter() {
            num += Self::item_spec(&f, boss_id, ty, acc)?;
        }
        Some(Value::F32(num))
    }

    /// One item's share, once the challenge/disabled flags are known to be off (see `equip_spec_value`).
    pub(super) fn item_spec(f: &EquipSpecFields, boss_id: i32, ty: i32, equip: &Value) -> Option<f32> {
        let Value::I32(first) = read_field(equip, f.spec_type[0])? else { return None };
        if first == 0 {
            return Some(0.0);
        }
        let Value::I32(required) = read_field(equip, f.boss_required)? else { return None };
        let ratio = boss_id as f32 / required as f32;
        let ratio = if ratio < 1.0 { ratio } else { 1.0 }; // Mathf.Min(a, 1f): NaN gives 1
        let mut num = 0f32;
        for i in 0..3 {
            let Value::I32(t) = read_field(equip, f.spec_type[i])? else { return None };
            if t == ty {
                let Value::F32(cur) = read_field(equip, f.spec_cur[i])? else { return None };
                num += (cur * ratio).floor();
            }
        }
        Some(num)
    }

    pub(super) fn equip_spec_value(&self, f: &EquipSpecFields, character: &Value, ty: i32, equip: &Value) -> Option<f32> {
        let [boss, settings, difficulty] = self.effective_boss_fields?;
        let no_equipment = read_field(&read_field(character, f.challenges)?, f.no_equipment)?;
        let Value::I32(in_challenge) = read_field(&no_equipment, f.in_challenge)? else { return None };
        if in_challenge != 0 {
            return Some(0.0);
        }
        let Value::I32(disabled) = read_field(&read_field(character, f.inventory)?, f.disabled)? else { return None };
        if disabled != 0 {
            return Some(0.0);
        }
        let Value::I32(first) = read_field(equip, f.spec_type[0])? else { return None };
        if first == 0 {
            return Some(0.0);
        }
        let Value::I32(required) = read_field(equip, f.boss_required)? else { return None };
        let Value::I32(mut boss_id) = read_field(character, boss)? else { return None };
        let Value::I32(difficulty) = read_field(&read_field(character, settings)?, difficulty)? else { return None };
        boss_id = boss_id.wrapping_add(301 * (i32::from(difficulty >= 1) + i32::from(difficulty >= 2)));
        let ratio = boss_id as f32 / required as f32;
        let ratio = if ratio < 1.0 { ratio } else { 1.0 }; // Mathf.Min(a, 1f): NaN gives 1
        let mut num = 0f32;
        for i in 0..3 {
            let Value::I32(t) = read_field(equip, f.spec_type[i])? else { return None };
            if t == ty {
                let Value::F32(cur) = read_field(equip, f.spec_cur[i])? else { return None };
                num += (cur * ratio).floor();
            }
        }
        Some(num)
    }

    pub(super) fn effective_boss_id(&self, base: usize) -> Option<Value> {
        let [boss, settings, difficulty] = self.effective_boss_fields?;
        let character = self.slots.get(base)?;
        let Value::I32(mut boss_id) = read_field(character, boss)? else { return None };
        let settings = read_field(character, settings)?;
        let Value::I32(difficulty) = read_field(&settings, difficulty)? else { return None };
        // Normal is 0, Evil is 1, Sadistic is 2. IL uses wrapping addition.
        boss_id = boss_id.wrapping_add(301 * (i32::from(difficulty >= 1) + i32::from(difficulty >= 2)));
        Some(Value::I32(boss_id))
    }

    pub(super) fn begin_titan_checks(&self, scope: u32, base: usize) -> Option<TitanChecks> {
        let [controller_player, player_settings, setting] = self.titan_check_fields?;
        let character = read_field(self.slots.get(base)?, controller_player)?;
        let settings = read_field(&character, player_settings)?;
        let Value::I32(enabled) = read_field(&settings, setting)? else { return None };
        let Value::Obj(character) = character else { return None };
        Some(TitanChecks { scope, character, enabled: enabled != 0, stats: [None; 3] })
    }

    pub(super) fn is_titan_stat_eligibility(&self, base: usize) -> bool {
        let Some(checks) = &self.titan_checks else { return false };
        let Some(Value::Obj(character)) = self.slots.get(base) else { return false };
        if !Rc::ptr_eq(character, &checks.character) { return false; }
        match self.callstack.as_slice() {
            [.., current] if *current == checks.scope => true,
            [.., parent, current] => *parent == checks.scope && self.prog.methods[*current as usize].guard == 7,
            _ => false,
        }
    }

}
