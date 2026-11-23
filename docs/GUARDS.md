# Interpreter shortcuts

The interpreter identifies the following game methods in `clr/src/program.rs` and applies shortcuts in `clr/src/vm.rs`. Set `NGU_NO_GUARD=1` before starting the interpreter to run their original instructions. The guards are an implementation detail of the interpreter and require behavioral comparisons against the original game.

| ID | Method selection | Purpose and existing coverage |
| --- | --- | --- |
| 1 | CookingController.updateDishUI | Skip redundant UI updates. |
| 2 | WishesController.progressPerTick | Return zero in the documented zero-allocation context. Covered by `wish_tick_preserves_levelups_and_idle_wishes`. |
| 3 | WishesController.updateAllWishes | Skip the loop when all wishes are idle. Covered by the same wish test. |
| 4 | AdventureController.manageFight | Scope repeated Titan calculations. Covered by `titan_checks_preserve_state_with_autokill_on_and_off`. |
| 5 | Character.totalAdvAttack | Reuse Titan eligibility calculation within the scoped fight. Covered by the Titan test. |
| 6 | Character.totalAdvDefense | Reuse Titan eligibility calculation within the scoped fight. Covered by the Titan test. |
| 7 | AdventureController.autokillTitan*Achieved | Skip eligibility checks where auto-kill is disabled within the scoped fight. Covered by the Titan test. |
| 8 | CookingController.getOptimalScore | Reuse the score for the same recipe. Covered by `cooking_uses_the_exact_score_after_recipe_changes`. |
| 9 | Character.totalAdvHP | Reuse health calculation within the scoped fight. Covered by the Titan test. |
| 10 | AllAchievementsController.checkAchievements | Scope repeated achievement calculations. Covered by `achievement_scan_preserves_thresholds_and_bonus_points`. |
| 11 | AllAchievementsController.achievementCondition | Scope achievement condition caching. Covered by the achievement scan test. |
| 12 | Character.totalEnergyPower | Cache repeated achievement-condition calculations. Covered by the achievement scan test. |
| 13 | Character.totalMagicPower | Cache repeated achievement-condition calculations. Covered by the achievement scan test. |
| 14 | Character.totalCapEnergy | Cache repeated achievement-condition calculations. Covered by the achievement scan test. |
| 15 | Character.totalCapMagic | Cache repeated achievement-condition calculations. Covered by the achievement scan test. |
| 16 | Character.totalEnergyBar | Cache repeated achievement-condition calculations. Covered by the achievement scan test. |
| 17 | Character.totalMagicBar | Cache repeated achievement-condition calculations. Covered by the achievement scan test. |
| 18 | Character.effectiveBossID | Native fast path with fallback. Covered by `boss_thresholds_and_loadout_shortcuts_match_the_interpreter`. |
| 19 | LoadoutController.Update | Skip shortcut processing when the relevant keys are absent. Covered by the boss and loadout test. |
| 20 | InventoryController.equipSpecBonus | Native fast path with fallback. |
| 21 | InventoryController.specBonus | Native fast path with fallback. |

The named tests use prepared game data and a suitable save. They compare specific scenarios and do not prove that all possible inputs behave identically. Guards without named tests in this table need targeted coverage. No decision about their long-term removal follows from this inventory.
