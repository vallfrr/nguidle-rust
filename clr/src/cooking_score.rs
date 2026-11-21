use crate::value::{Ext, Value};
use crate::vm::{field_ref, read_field, Vm};

#[derive(PartialEq, Eq)]
pub(crate) struct Recipe {
    pairs: [[i32; 3]; 4],
    ingredients: [[[u32; 4]; 2]; 4],
}

pub(crate) struct ScoreFields {
    character: (u32, usize),
    cooking: (u32, usize),
    ingredients: (u32, usize),
    pairs: [(u32, usize); 4],
    targets: [(u32, usize); 4],
    ingredient: [(u32, usize); 4],
}

impl ScoreFields {
    pub(crate) fn resolve(vm: &Vm) -> Option<Self> {
        let cooking = |name| field_ref(vm, "Cooking", name);
        let ingredient = |name| field_ref(vm, "Ingredient", name);
        Some(Self {
            character: field_ref(vm, "CookingController", "character")?,
            cooking: field_ref(vm, "Character", "cooking")?,
            ingredients: cooking("ingredients")?,
            pairs: [cooking("pair1")?, cooking("pair2")?, cooking("pair3")?, cooking("pair4")?],
            targets: [cooking("pair1Target")?, cooking("pair2Target")?, cooking("pair3Target")?, cooking("pair4Target")?],
            ingredient: [ingredient("unlocked")?, ingredient("targetLevel")?, ingredient("weight")?, ingredient("pairedWeight")?],
        })
    }

    pub(crate) fn recipe(&self, controller: &Value) -> Option<Recipe> {
        let character = read_field(controller, self.character)?;
        let cooking = read_field(&character, self.cooking)?;
        let Value::Obj(ingredients) = read_field(&cooking, self.ingredients)? else { return None };
        let ingredients = ingredients.x.try_borrow().ok()?;
        let Ext::List(ingredients) = &*ingredients else { return None };
        let mut key = Recipe { pairs: [[0; 3]; 4], ingredients: [[[0; 4]; 2]; 4] };
        for pair in 0..4 {
            let Value::I32(target) = read_field(&cooking, self.targets[pair])? else { return None };
            let Value::Obj(pair_entries) = read_field(&cooking, self.pairs[pair])? else { return None };
            let pair_entries = pair_entries.x.try_borrow().ok()?;
            let Ext::List(pair_entries) = &*pair_entries else { return None };
            let [Value::I32(a), Value::I32(b)] = pair_entries.as_slice() else { return None };
            key.pairs[pair] = [*a, *b, target];
            for (side, index) in [*a, *b].into_iter().enumerate() {
                let ingredient = ingredients.get(usize::try_from(index).ok()?)?;
                for (slot, field) in self.ingredient.into_iter().enumerate() {
                    key.ingredients[pair][side][slot] = match (slot, read_field(ingredient, field)?) {
                        (0 | 1, Value::I32(value)) => value as u32,
                        (2 | 3, Value::F32(value)) => value.to_bits(),
                        _ => return None,
                    };
                }
            }
        }
        Some(key)
    }
}
