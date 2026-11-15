# Operation Body Count's map archive, as implemented

The authority is OBCR's reader, `crates/obc-formats/src/maps.rs`; the
reverse-engineering record is `docs/re/DATA_FORMATS.md` section 3. Everything
here was checked by parsing all 61 maps of both releases (they are identical)
and re-encoding them byte for byte (`obcedit verify-archive`). All multi-byte
fields are little-endian.

## MAPHEAD.BC (402 bytes)

| Offset | Type | Meaning |
| --- | --- | --- |
| 0 | u16 | RLEW tag, 0xABCD |
| 2 | i32 × 100 | Offset of each slot's map header in MAPTEMP.BC; **-1** for an empty slot (the engine treats any value ≤ 0 as empty) |

Slots are 0-based and are the engine's `mapon`: slot 0 is floor 1. The original
uses 0–60: 0–39 the campaign, 40–49 unused credit rooms, 50–59 the network
arenas, 60 an empty map.

## MAPTEMP.BC

```
"TED5v1.0"                       8 bytes
for each used slot:
    header                       38 bytes
      0   i32 × 3   absolute offsets of planes 0, 1, 2
      12  u16 × 3   stored length of each plane
      18  u16       width   (64)
      20  u16       height  (64)
      22  char × 16 name, NUL-terminated; bytes after the NUL are kept as read
    plane 0, plane 1, plane 2
    "!ID!"
```

Each plane is a u16 expanded length in bytes (8,192 for 64×64) followed by RLEW
words: the tag, a count and a value expand to `count` copies of the value; any
other word is itself. A literal equal to the tag must be written as a run. The
original's encoder never writes a run shorter than four, and neither does
OBCEdit's, which is what makes a re-encode identical.

**Order.** Maps follow in slot order, except that the original stores slot 56
after slot 60. The engine only follows offsets; OBCEdit's writer keeps every
map that came from a file in its original place and appends new maps in slot
order.

**Plane 2** is all zero in every original map; it is preserved as read.

## What the words mean

`docs/re/LEVEL_CODES.md` is the full record. In short:

| Plane 0 | |
| --- | --- |
| 1–129 | Walls; texture page = word − 1. Special: door-look walls 13/29/49/65/69 (+3 animation frames each), glass 57–60 and 117–120, fan 73–76, damaged 77, rock opening 88, generator 95/96, exits 113 ladder / 114 stairs / 115 elevator |
| 130–137 | Doors: look = (word − 130) / 2, even = vertical (walk through east-west), odd = horizontal |
| 138 | Ambush marker under a standing enemy |
| 140–161 | Floor; area = word − 140; floor/ceiling texture set = word − 144 when 0–9 |

| Plane 1 | |
| --- | --- |
| < 19 | Ignored (18 is the empty background) |
| 19–22 | Player start facing north, east, south, west |
| 23–85 | Static objects (`statinfo[word − 23]`), pickups among them |
| 86–88, 103, 106, 107, 124 | Animated-wall behaviour (fans, sliding panels, open passages) |
| 89 | Explosion-proof wall |
| 90–97 | Patrol turn arrows, east counter-clockwise to southeast |
| 98 | Push wall (moves exactly two cells) |
| 104, 105 | See-through wall (window, grille) |
| 108–317 | Actors in three difficulty tiers, allies 224–227 (+18, +36), fire 282–285; the catalog lists every one |
