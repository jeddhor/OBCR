# GitHub issues #16 and #17 — plan and findings

**Milestone M20.** Status: implemented 2026-11-20 on `m20/issues-16-17`.

Two issues were reported against 1.0.0-beta.5. As in [ISSUES_M19.md](ISSUES_M19.md), each is classified first: is it an OBCR defect (it differs from the original) or the original's own behaviour (DESIGN §4.3)?

## #16 — the GAMEPLAY FIXES menu is not readable

**Report.** OPTIONS > GAMEPLAY FIXES draws overlapping text (screenshot in the issue); the options themselves work.

**Plan.**

1. Measure the page against the game font: the label, its state and its description against the 304-pixel box.
2. Lay it out so nothing can overflow, whatever the font.
3. Test it the way the Options screen's text is tested: every string checked against the real font, and the drawn page checked for pixels outside the box.

**Finding (CONFIRMED, OBCR defect).** M19 drew each fix on one line: the quirk number and label at x 20, FIXED or ORIGINAL at x 236, and the description beneath. The game font averages about 9.5 pixels a character, so:

- labels of 22 or more characters ran into the state;
- 36-character descriptions and the footer ran past the box.

The Options screen had a font-fit test, but this page did not.

**Fix.**

- Each fix takes two lines: its name, then `K-001  FIXED` or `K-001  ORIGINAL`.
- The selected fix's description appears at the bottom, wrapped by measured pixel width to two lines, with *APPLIES TO NEW GAMES* beneath. The title is centred, and the list scrolls beyond five fixes.
- Test: `menu::tests::gameplay_fixes_text_fits_the_box` (data-gated) checks:
  - every label, state and description fits in the game font, with no word of a description cut;
  - drawing the page for every selection and both states leaves no pixel outside the box.

## #17 — push-wall textures change alignment

**Report.** Some push walls, not all, change their texture alignment when they start to move. In DOS, at the same place, they do not. The report includes a save and was made in Enhanced, FLOORS SMOOTH, no upscaling.

**Plan.**

1. Compare OBCR's push-wall hit code with the original's `HitVertPWall` and `HitHorizPWall` (RENDERER.md §4).
2. Fix the difference.
3. Test that a push wall starting to move looks exactly like the wall it was, on all four faces.

**Finding (CONFIRMED, OBCR defect).**

- `HitVertPWall` (1b4a:0ade) mirrors the texture (`0xFC0 − texture`) when `xtilestep == −1`, as `HitVertWall` does.
- `HitHorizPWall` (1b4a:09f3) mirrors it when `ytilestep != −1`.
- OBCR's vertical push-wall face mirrored on the opposite condition (`xtilestep != −1`). So a push wall seen on its east or west face flipped its texture horizontally the moment it began to move.
- Push walls seen on north or south faces were right, which is why only some walls showed it, and only those with asymmetric textures.
- The fault was in the shared ray caster, so Classic had it too.

**Fix.**

- `View3D::hit`: the vertical push-wall face mirrors when `xs == −1`.
- Test: `view3d::tests::a_push_wall_keeps_its_texture_when_it_starts_to_move` draws a column-coded texture as a still wall and as a push wall at `pwall_pos` 0, facing east, north, west and south. It fails on the east face without the fix.

## Also

- #9 (far creature sounds, from M19): the reporter confirms beta 5 fixes it.
