# Interpreter shortcuts

The interpreter identifies the following game methods in `clr/src/program.rs` and applies shortcuts in `clr/src/vm.rs`. Set `NGU_NO_GUARD=1` before starting the interpreter to run their original instructions. The guards are an implementation detail of the interpreter and require behavioral comparisons against the original game.

| ID | Method selection | Purpose and existing coverage |
| --- | --- | --- |
| 1 | CookingController.updateDishUI | Removed after guarded and unguarded replays showed that off-screen UI text diverged. |
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
| 20 | InventoryController.equipSpecBonus | Native fast path with fallback. Covered for the saved weapon and all 52 bonus types, enabled and disabled inventory, by `inventory_bonus_shortcuts_match_interpreter_for_all_types`. |
| 21 | InventoryController.specBonus | Native fast path with fallback. Covered for equipped items and all 52 bonus types, enabled and disabled inventory, by the same test. |

The named tests use prepared game data and a suitable save. They compare specific scenarios and do not prove that all possible inputs behave identically. Guard 1 was removed because the skipped updates changed the stored UI state. The remaining guards need broader evidence.

Unity RNG vectors captured from the original Unity 2019.4.22f1 runtime are checked in `clr/tests/fixtures/unity2019_rng.csv`. See [the probe](../tools/oracle/README.md) for the collection method and coverage.

## Evidence, 27 November 2026 (D4)

Every row below comes from local runs on the real game (PC, native). `tools/longrun.sh` plays the same long game with every shortcut, with none, and (with `NGU_GUARD_TABLE=1`) once per shortcut with only that one disabled. The **long run** columns use the maintainer's early PC save with 2 rounds of 150 pseudo-random clicks and 20 game minutes each; the **console** columns use `tools/guard-frames.sh` on the console's `session` scenario replayed through the C interface (the cost when only that shortcut is disabled, in total instructions, in the 99th percentile frame and in the worst frame). Machine-independent: instruction counts. Not measured on the console itself.

| ID | Answers in the long run | Long run, final save without this shortcut | Long run, extra instructions without it | Console, extra total / p99 frame / worst frame without it |
| --- | --- | --- | --- | --- |
| 2 | 0 | identical | 0 % | 0 % / 0 % / 0 % |
| 3 | 128,584 | identical | +0.06 % | +0.06 % / +0.03 % / 0 % |
| 4 | 77,150 | identical | +7.5 % | 0 % / 0 % / 0 % |
| 5, 6, 7 | 0 | identical | 0 % | 0 % / 0 % / 0 % |
| 8 | 10,282 | identical | +365 % | +445 % / +5,255 % / +280 % |
| 9 | 77,078 | identical | +7.5 % | 0 % / 0 % / 0 % |
| 10 | 2,573 | identical | +7.1 % | +8.0 % / +94 % / +6.6 % |
| 11 | marker for 12 to 17 | identical | 0 % | 0 % / 0 % / 0 % |
| 12 to 17 | 38,595 each | identical | +1.2 % each | +1.3 % / +15.7 % / +1.1 % each |
| 18 | 3,475,074 | identical | +3.8 % | +0.21 % / +0.6 % / +0.04 % |
| 19 | 3,626,050 | identical | +6.0 % | +6.7 % / +2.6 % / +0.19 % |
| 20 | 0 | identical | 0 % | 0 % / 0 % / 0 % |
| 21 | 18,564 | identical | +0.17 % | +0.06 % / 0 % / 0 % |

Also identical, with and without all shortcuts: 9 rounds of the same long game (3 game hours, two hours of offline progress in the middle, 124,460 bytes), and 3 rounds from a new game with every feature unlocked (125,884 bytes). The seven named tests above pass on a save produced by the port after 8 game hours (`NGU_TEST_SAVE`, with `NGU_TEST_DLL` and `NGU_TEST_ASSETS`, `cargo test --release -p clr --test wish_tick -- --ignored`); they build the states the long run does not reach (wishes, Titan auto-kill, equipped items).

What this does not show: shortcuts 2, 5, 6, 7 and 20 answered nothing in any long run, and 4 and 9 only in the unlocked-game run, so their gain on a late-game save is unmeasured here; shortcuts 3 and 21 are worth less than 0.2 % of the instructions on every scenario tried. D4 removes a shortcut that shows no gain on the real console. Nothing is removed yet: that needs a late-game save on the console, or the maintainer's decision. No shortcut changed a save in any run.
