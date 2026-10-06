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
| Left / right mouse (hold) | Grab with that hand; holding the button grabs as soon as a hold is in reach |
| W / S | Pull up / lower yourself while hanging; walk on the ground |
| A / D | Shift sideways |
| Space | Jump (hands let go and pay a grip cost) |
| Shift | Sprint on the ground; slide freely along the wall while hanging |
| Ctrl | Brace: clamp to the wall and kill your swing |
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
handholds, crouch height, swimming, fall damage, and the game's sprite hands
and audio.

## Where the numbers come from

`src/tuning.rs` holds the values the game's player object is actually
configured with, which differ a lot from the defaults in its scripts (gravity
is -0.2, not -9.81, because velocity is multiplied by 175 when applied). The
handhold values are the script defaults; individual handholds in the game may
override them.

## Self-test

`WK_SELFTEST=1 cargo run` plays a short scripted climb (walk to the wall, grab,
pull up, second hand, jump) and logs the body's position and grip each quarter
second, then exits.
