# Screen profiles

The named `vita` profile is an experimental region layout for PC captures.
The actual [Vita preview](../docs/VITA.md) reflows the shared layout across
the 960 × 544 screen, with L and R switching menus.

The game always draws into its own 960x600 space. A profile cuts that space into **regions** and places each one on the real
screen with its own scale; the pointer is mapped back through the same regions, so the game code is unchanged.
Code: `clr/src/profile.rs` (`pc` and `vita` are built in; any other value is read as a file path).

One line per region (later lines are drawn on top and get the pointer first):

```
region <name> <x> <y> <w> <h> <place> [flags] nodes=<node>,<node>...
```

- `x y w h`: the region's rectangle in the 960x600 game space (top-left origin). Used to map the pointer and the focus frame.
- `place`:
  - `left`: next to the previous `left` regions, scaled to the screen height;
  - `over`: at the left edge, scaled to the height, without taking room (shown in place of another region);
  - `rest`: the remaining width; if that makes it taller than the screen, it pans vertically with the pointer;
  - `fit`: the whole screen, fitted and centred.
- flags:
  - `hold` / `!hold`: visible only while / except while the profile hold action is pressed (F2 in the PC window);
  - `follow`: drawn with the region under the mouse (tooltips);
  - `when-drawn`: takes the pointer only while something of it is on screen (pop-ups).
- `nodes`: UI objects drawn in this region: names of children of the root `Canvas`, or `Parent/Child` one level deeper.
  `*` = everything not bound elsewhere (exactly one region must have it).
  `clr`'s dev step `tree,2` lists the names with their rectangles.

Try one: `NGU_LAYOUT=vita` with the PC window (`ngu-window ...`), or the script step `pngl,<out.png>,<profile>,<w>,<h>[,hold][,pointer y]`.
