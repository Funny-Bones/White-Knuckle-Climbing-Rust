# White Knuckle climbing, in Bevy

A Rust/Bevy reimplementation of the two-handed climbing from *White Knuckle*:
each mouse button is a hand, each hand has its own grip stamina, and you move
by grabbing handholds, pulling yourself up and jumping between them.

```bash
cargo run
```

## Controls

| Input | Action |
| --- | --- |
| Left / right mouse (hold) | Grab with that hand; holding the button grabs as soon as a hold or a ledge is in reach |
| W / S | Pull up / lower yourself while hanging; walk on the ground |
| A / D | Shift sideways |
| Space | Jump (hands let go and pay a grip cost) |
| Shift | Sprint on the ground; slide freely along the wall while hanging |
| Ctrl | Crouch on the ground; brace (clamp to the wall and kill your swing) while hanging |
| R | Back to the start |
| Esc | Release the mouse (click to capture it again) |

## What is ported

- Per-hand grabbing with the original's aim assist (ray, swept sphere, and a
  0.2 s "grab memory" after the crosshair leaves a hold).
- Per-hand grip: 10 s on one hand, drained at half rate when both hands hold,
  recovering on the ground and (after a short delay) in the air. An exhausted
  hand lets go and is locked out for a second.
- The hang model: holding on bleeds off vertical speed (more firmly the longer
  you have held), arms pull you in once you are further than 1.2 m from the
  hold, W drives you up the surface you are pressed against, and a hold is lost
  if you get dragged to twice your reach.
- Ground and air movement in the original's velocity units, including its
  quadratic drag, coyote time, wall friction and steep-slope sliding.
- Per-handhold tuning (`Handhold` in `src/level.rs`): strain rate, jump
  multiplier, climb multiplier, drag and so on. The red holds in the test
  level burn grip 2.5x faster; the green one launches a stronger jump.

Not ported: items and tools (pitons, rebar, hammer), perks and buffs, moving
handholds, swimming, fall damage and voice lines.

## Ledge grabbing

Not from the original: a hand can also grab the top edge of any solid
structure. Look at a wall just below its top (or slightly over the edge) and
hold a mouse button; the aim dot shows the grip point. A ledge behaves like a
standard handhold. Pulling up only works while looking level or upward, as in
the original: looking down with W held lowers you instead.

## The test park

The level (`src/level.rs`) is a flat park of white structures on a gridded
floor (one square per metre) under a clear blue sky, all generated in code. You start facing the main
wall; everything else is a short walk away.

| Structure | Where | What it tests |
| --- | --- | --- |
| Main wall | north | Zigzag, rest ledge, slick holds, spring jump, roof and lip |
| Chimney | north-east | Crossing between two facing walls |
| Ladder spire | far north-east | A 50 m climb that needs its rest ledges |
| Twin towers | far east | All-slick holds; widely spaced spring holds |
| Leaning slab | east | An overhang on one side, a lean-back climb on the other |
| Hanging garden | south-east | Bars floating at mixed heights to swing between |
| Traverse wall | south-east | A long sideways traverse above the ground |
| Monkey bars | south | Hand over hand under a beam |
| Spiral islands | far south | Floating platforms with a bar in each gap |
| Arch | south-west | Up one leg, under the top, down the other |
| Stepping blocks | south-west | Jumps of rising height |
| Ramps | west | A walkable slope and a steep one to slide down |
| Pillar forest | west | Nine pillars wrapped in spirals of holds |
| Great cube | north-west | Holds on every side and under an overhanging cap |

Yellow holds are normal, red ones burn grip 2.5x faster, green ones launch a
stronger jump. `WK_OVERVIEW=1 cargo run` saves bird's-eye pictures of the park
to `target/overview_*.png`.

## Hand sprites

The hands are drawn with the game's own sprites if these files exist, and as
plain boxes otherwise:

```
assets/original/hand_background_normal.png
assets/original/hand_background_open.png
assets/original/hand_background_grab.png
```

Two more optional files add the HUD icons beside the crosshair, which show an
open hand when a hold or ledge is in reach and a fist while holding on,
reddening as that hand's grip runs down:

```
assets/original/hud_hand_open.png
assets/original/hud_hand_grabbed.png
```

They are the 128x128 cells of the game's `Hands_Background_Sprite_Library_01`
texture. That art belongs to the game, so `assets/original` is git-ignored and
is not part of this repository; extract it from your own copy.

## Sounds

Grabbing, letting go, losing your grip, jumping, mantling, landing, footsteps
(faster when sprinting, slower when crouched) and the fatigue loop each play the game's own clip, at the volume and pitch range the
game uses, if the clip exists as a WAV in `assets/original/audio/`:

```
climb1-4  letgo1-3  player_climbing_griplost  hand_fatigue_new
player_step_concrete_01,03  player_step_generic_01-03
player_fem_jump1-3  mantle1  landsoft2_new  land_medium1-3
player_fem_land_heavy_01-02
```

Like the sprites, these belong to the game and are git-ignored. Any clip that
is missing is simply skipped, so the game runs silent without them.

## Where the numbers come from

`src/tuning.rs` holds the values the game's player object is actually
configured with, which differ a lot from the defaults in its scripts (gravity
is -0.2, not -9.81, because velocity is multiplied by 175 when applied). The
handhold values are the script defaults; individual handholds in the game may
override them.

## Self-test

`WK_SELFTEST=1 cargo run` plays a short scripted climb (walk to the wall, grab,
pull up, second hand, jump) and logs the body's position and grip each quarter
second, then exits. It also saves three screenshots to `target/selftest_*.png`.

`WK_WALK=1 cargo run` walks and sprints around open ground for 40 seconds and
reports how many times the body lost its speed without being told to stop.

`WK_LEDGE=1 cargo run` grabs the top of a three metre block, pulls up and over,
and reports whether the body ended up standing on it.

`WK_HANDS=1 cargo run` photographs the hand poses (rest, walk, run, reach,
crouch) to `target/hands_*.png`. Adding `WK_NOVSYNC=1` to any run uncaps the
frame rate, and the self-test then reports the real rendering speed.
