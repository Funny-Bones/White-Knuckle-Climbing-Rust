# Roadmap

Where the port stands and what comes next, in the order I would do it. Each
stage is meant to leave the game playable.

## Done

- Two-handed climbing: grab assist, per-hand grip, the hang and pull-up model,
  jumps off holds, ground and air movement, all on the original's tuning.
- Per-handhold tuning (strain, jump, climb, drag multipliers).
- Sprite hands on an overlay camera, with fist / open / relaxed poses.
- Sounds for grab, let go, jump, mantle, landings and the fatigue loop.
- Greybox test wall and a scripted self-test with logs and screenshots.

## 1. Make it feel right

Nothing so far has been tuned by hand; it has only been checked by script.
This stage is playing it next to the original and closing the gaps.

- [ ] Play the whole test wall. The ledge traverse, red and green holds, roof
      crossing and top lip have never been climbed.
- [ ] Compare against the original: hang height on one and two hands, pull-up
      speed, jump height, wall slide, and the sideways swing while pulling up
      one-handed (seen in the self-test, cause unknown).
- [ ] Set the sound level by ear (`MASTER_GAIN`) and check each clip sounds
      clean.
- [ ] Read the remaining configured values from the game data instead of using
      script defaults: landing speed cutoffs, mouse sensitivity, and the
      per-handhold overrides on real handholds.
- [ ] Port the hand animation rules (not read yet), and use the crouch and
      sprint poses that are already extracted.
- [ ] Camera feel: view sway, shake when grip is nearly gone, sprint FOV.
- [ ] Recover the grip-lost sound, the one clip stored in a codec the
      extractor cannot decode.

## 2. Finish the body

Parts of the original player that were left out of the first pass.

- [ ] Crouch height and the crouched capsule.
- [ ] Moving handholds: carry the hold's velocity into the body, and ride
      moving platforms.
- [ ] Footsteps, and landing effects on the camera.
- [ ] Health, fall damage and death, with respawn.
- [ ] The resting state on a wall and its effect on grip.

## 3. Handhold variety

The original attaches behaviours to handholds; the port has only the tuning
numbers.

- [ ] Breakable, icy, damaging and timed handholds.
- [ ] Ropes.
- [ ] Direction-limited holds and jump-boost holds.
- [ ] Surface types with their own grab sounds (chain, fungus).

## 4. Tools

The largest missing piece of the climbing itself.

- [ ] Items held in a hand, and a hand that is busy holding one.
- [ ] Hammer, and pitons that must be hammered in before they are secure.
- [ ] Rebar and other placeable holds.
- [ ] A minimal inventory to carry them.

## 5. Somewhere to climb

- [ ] A level format loaded from a file instead of code.
- [ ] A longer course that mixes the handhold types.
- [ ] Procedurally placed handholds on a wall.

## 6. Project upkeep

- [ ] Put the asset extraction scripts in the repository. They currently exist
      only in a temporary folder, so nobody else can reproduce the sprites and
      sounds, and they will be lost.
- [ ] Unit tests for the movement maths that run without a window.
- [ ] Build and test on every push.
- [ ] Tuning values loadable from a file, so they can change without a rebuild.

## Not planned

Perks and buffs, enemies, swimming, and the game's levels. The game's art and
audio stay out of the repository; they are loaded from a local copy only.
