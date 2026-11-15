# Options, the command line and the settings file

Everything you can change in OBCR, and where it is kept. Most of it is in the **OPTIONS** screen; the rest is on the command line or in `obcr.toml`.

- [The original's menu settings](#the-originals-menu-settings)
- [OPTIONS](#options)
  - [Display](#display)
  - [Textures and floors](#textures-and-floors)
  - [Controls](#controls)
  - [Sound and music](#sound-and-music)
- [The command line](#the-command-line)
- [The settings file](#the-settings-file)
- [Where OBCR keeps things](#where-obcr-keeps-things)

---

## The original's menu settings

These are the original game's own settings, in its main menu. OBCR keeps them as the original did and saves them with its other settings.

| Item | What it does |
|---|---|
| **VIEW SIZE** | The size of the 3D view inside the frame, in five steps (12, 14, 16, 18, 20; default 18). The status bar is always shown. |
| **GAME TYPE** | **1**, **2**, **3**: difficulty ("for novices", "for those who have played before", "Uhhh, this is hard"). **R**: Random, where enemies and items are scattered from where the map puts them. **N**, **T**, **M**: network deathmatch, network teams and the two-player direct game ([multiplayer.md](multiplayer.md)). What each changes is in the [game guide](game-guide.md#game-types). |
| **AUDIO MIX** | Opens OPTIONS at its sound settings. |
| **D** (MODS) | Choose the game START GAME plays: the original or an installed mod ([modding.md](modding.md)). |
| **F5 / F6** in game | Textured floors and ceilings off / on (the original's `floormode`). Off draws them in flat colours, as the original's low-detail setting did. |

---

## OPTIONS

Press **O** in the main menu. Up and Down choose an option and Left and Right change it; changes apply at once and are saved when you quit. **Enter** on KEY BINDINGS opens the bindings screen. **Esc** goes back.

Some options apply only in **Enhanced** mode. **Classic** always shows the original picture, whatever they are set to.

### Display

| Option | Values (default in bold) | What it does |
|---|---|---|
| **DISPLAY** | CLASSIC 320X200, **ENHANCED** | Classic is the original picture, pixel for pixel, scaled up to your window. Enhanced renders at a higher resolution and allows everything below. |
| **RESOLUTION** | 2X to 8X (**4X**) | Enhanced: the multiple of the original 200 lines the game renders at. 4X is 800 lines, 6X is 1200. |
| **ASPECT** | 4:3, 16:10, **16:9**, 21:9 | Enhanced: the shape of the picture. Wider shapes show more to the sides (Hor+), as Wolfenstein 3D ports do. 4:3 is the original's shape. |
| **FULLSCREEN** | ON, **OFF** | Fullscreen instead of a window. Alt+Enter toggles it at any time. |
| **SMOOTH FILTER** | ON, **OFF** | Bilinear filtering when the picture is stretched to the window. Off keeps pixels sharp. |
| **INTEGER SCALING** | ON, **OFF** | Scale only by whole multiples, with black borders, so every original pixel is the same size. |
| **SMOOTH MOTION** | **ON**, OFF | Enhanced: draws the view and every actor between game tics, so motion is as smooth as your monitor's refresh rate. The game itself still runs at the original's 70 tics per second (35 frames). Off shows only the frames the game computes. |

### Textures and floors

| Option | Values | What it does |
|---|---|---|
| **TEXTURES** | ORIGINAL, XBRZ 2X to XBRZ 6X (**4X**) | Enhanced: upscales walls, doors, floors, ceilings, enemies, items and the hand weapon with xBRZ, which rounds off blocky texels while keeping edges sharp. The upscaling works on palette indices, so shading, damage and bonus flashes, fades and the generators-off darkness still work as in the original. Textures are built when first needed, about 0.2 MB each at 4X and 0.4 MB at 6X. |
| **FLOORS** | **SMOOTH**, ORIGINAL | Enhanced: SMOOTH draws floors and ceilings in true perspective, continuous across tiles, with the light fading smoothly with distance. ORIGINAL keeps the original's look: each tile drawn separately, the texture pasted flat on screen, a grout line between tiles and one brightness per tile. |
| **XBRZ MENUS** | **ON**, OFF | Enhanced: upscales the menus, the title and story pictures, the animated sequences and movies, and the help and score pages, at the resolution multiple (up to 6X). |
| **XBRZ STATUS BAR** | ON, **OFF** | Enhanced: upscales the in-game status bar and overlays too. Off by default: xBRZ distorts its small lettering. |

### Controls

| Option | Values | What it does |
|---|---|---|
| **CONTROL SCHEME** | **MODERN**, ORIGINAL | Modern: WASD, mouse look, Space or E to use. Original: the arrow keys, Alt to sidestep, Space to use, and the original's mouse (the right button runs and walks forward, the middle button uses). Changing the scheme resets the key bindings to that scheme's defaults. |
| **KEY BINDINGS** | CHANGE... | The bindings screen (below). |
| **ALWAYS RUN** | ON, **OFF** | Run all the time; the run key then walks. |
| **MOUSE SPEED** | 25 to 400 (**100**) | Mouse turning speed, in percent. |
| **PAD TURN SPEED** | 25 to 300 (**100**) | Gamepad right-stick turning speed, in percent. |
| **RUMBLE** | **ON**, OFF | Gamepad rumble when you are hurt, at explosions and when the screen shakes. |

**The KEY BINDINGS screen** shows one device at a time: Left and Right switch between *KEYBOARD AND MOUSE* and *GAMEPAD*.

- **Enter** (or the gamepad's A) waits for the new input; press a key, a mouse button, the wheel or a gamepad button. After 5 seconds, or on Esc or Start, it gives up.
- **Backspace** or **Delete** (or X) clears the row.
- Each action takes up to **three** keyboard or mouse inputs and **two** gamepad buttons. Binding one more drops the oldest.
- An input already used by another action moves to the new one, and the screen says which action lost it.
- The last row, *RESET TO … DEFAULTS*, restores the current scheme's defaults.
- **Esc**, **B** or **Back** returns.

What can be bound: every key; the left, right and middle mouse buttons, buttons 4 and 5 and the wheel (up and down); and the gamepad's A, B, X, Y, the bumpers, both triggers, Back, Start, both stick clicks and the D-pad. Some inputs are not bindings and always work. The sticks move and turn (left stick moves and sidesteps, right stick turns), the mouse turns in the modern scheme, and **Esc** and **Start** open the menu.

The actions, with the default inputs of each scheme:

| Action | Modern | Original | Gamepad |
|---|---|---|---|
| FORWARD / BACK | W, ↑ / S, ↓ | ↑ / ↓ | D-pad up |
| TURN LEFT / TURN RIGHT | ← / → | ← / → | D-pad left / right |
| STRAFE LEFT / STRAFE RIGHT | A / D | — | — |
| STRAFE HOLD (turn keys sidestep while held) | — | Alt | — |
| RUN | Left Shift, Right Shift | Left Shift, Right Shift, right mouse | LT, L3 |
| FIRE | Ctrl, left mouse | Ctrl, left mouse | RT |
| USE | Space, E, right mouse | Space, middle mouse | A |
| WEAPON 1–5 | 1–5 | 1–5 | — |
| NEXT / PREV WEAPON | wheel up / down | wheel up / down | RB / LB |
| ALLY SWITCH | Q | A | X |
| STAY+SWITCH | Z | S | B |
| ALLY MODE (follow / disperse) | F | F | Y |
| WIPE VISOR | V | W | D-pad down |
| MAP | Tab | Tab | Back |
| CROSSHAIR, HEALTH | C, H | C, H | — |
| TREMORS | F8 | F8 | — |
| PAUSE | Pause | Pause | — |
| QUICK SAVE | F9 | F9 | — |
| SOUND, MUSIC | F1, F2 | F1, F2 | — |
| FLOORS OFF, FLOORS ON | F5, F6 | F5, F6 | — |
| MENU | Esc | Esc | Start |
| QUIT TO DOS | F10 | F10 | — |

Fixed keys that are not in the list: **Alt+Enter** (fullscreen); with the CD release's music, **Alt+P**, **Alt+S** and **Alt+F** (play, stop, next track); and, with `--nsr250`, the original's bogus-mode cheats on Alt ([game guide](game-guide.md#cheats)).

### Sound and music

| Option | Values | What it does |
|---|---|---|
| **MASTER VOLUME** | 0 to 100 (**80**) | Everything. |
| **SOUND FX VOLUME** | 0 to 100 (**100**) | Effects, digitized and synthesized. |
| **MUSIC VOLUME** | 0 to 100 (**70**) | AdLib music and CD music. |
| **MUSIC SOURCE** | **CD AUDIO**, ADLIB | CD release with ripped music: CD AUDIO plays the CD's four tracks as the CD version did; ADLIB plays the AdLib (FM) music instead. Without ripped music this shows ADLIB (NO CD). |
| **CD TRACK END** | **NEXT TRACK**, ORIGINAL | What happens when a CD track ends. The original fell silent until the next floor, then played the same track again; NEXT TRACK goes on to the next of the four. See [cd-release.md](cd-release.md). |
| **SOUND VOICES** | 1 ORIGINAL, **4 MIXED** | 1 plays one digitized sound at a time with the original's priorities, as a Sound Blaster did. 4 mixes up to four, so a shot does not cut off a scream. |
| **SOUND EFFECTS** | **DIGITIZED**, ADLIB, PC SPEAKER | The effects device, as in the original's sound setup: digitized samples, AdLib (FM) effects, or the PC speaker. |

---

## The command line

```
obcr [OPTIONS]
```

### Game data

| Option | What it does |
|---|---|
| `--data DIR` | The folder with the game files. Without it, OBCR tries the `OBC_DATA_DIR` environment variable; `data_dir` in the settings file; the folders `OBC`, `data` or the executable's own folder next to the executable; then Steam's default `steamapps/common/Operation Body Count/C/BCCD`. |
| `--check-data DIR\|IMAGE` | Identify a copy of the game (a folder, or a `.cue`, `.bin` or `.iso` image) and exit. Prints one tab-separated line per file and per CD track. Exit code 0 if every required file matches a known release, 2 if not. Setup uses this. |
| `--extract-data SOURCE DIR` | Copy the files OBCR uses from a folder or CD image into DIR, ripping the CD music from an image, and exit. Prints progress lines. Setup uses this; it works on its own too. |

### Display

| Option | What it does |
|---|---|
| `--classic` | Classic 320×200 for this run. |
| `--scale N` | Enhanced at N× (2–8). |
| `--aspect A` | Enhanced aspect: `4:3`, `16:10`, `16:9`, `21:9`. |
| `--xbrz N` | Texture xBRZ factor, 2–6, or 0 for the original texels. |
| `--xbrz-menus on\|off`, `--xbrz-hud on\|off` | xBRZ for the menus and pictures, and for the status bar. |
| `--floors smooth\|original` | Smooth or original floors and ceilings. |
| `--fullscreen`, `--windowed` | Start fullscreen or in a window. |

These change the settings as if you had chosen them in OPTIONS, and they are saved.

### Mods

| Option | What it does |
|---|---|
| `--mod PATH\|ID` | Play a mod for this run: a mod folder, a `.obcmod` file, or an installed mod's id. |
| `--check-mod PATH` | Check a mod and exit: one tab-separated line per finding, then `result ok` or `result errors`; exit code 0 or 2. OBCEdit uses this. |
| `--start X,Y,DIR` | With `--play`: start the player at cell X,Y (0-63) facing DIR (0 north, 1 east, 2 south, 3 west). OBCEdit's *play from here*. |
| `--convert-audio IN.wav OUT.qoa` | Convert a WAV file to QOA music for a mod (mono or stereo, 8-96 kHz) and exit; exit code 0 or 1. OBCEdit uses this when you import WAV music. |

### Settings and the original's parameters

| Option | What it does |
|---|---|
| `--config FILE` | Use FILE as the settings file; saved games then go to a `saves` folder beside it. Handy for a portable copy on a USB stick. |
| `--nsr250` | The original's cheat parameter: in game, Alt+Left Shift+B turns on BOGUS MODE ([game guide](game-guide.md#cheats)). Bogus games never enter the high scores. |

### Network games

| Option | What it does |
|---|---|
| `--net N\|T\|M` | Start a network game at once: N deathmatch, T teams, M two players. |
| `--net-setup` | With `--net`: open the setup screen instead of starting at once. |
| `--net-join HOST[:PORT]` | Call this player (repeatable). |
| `--net-port P` | UDP port (default 16387, the original's IPX socket 4003 hex). |
| `--net-choice 0\|1\|2` | Character (N, M) or team (T). |

Details and examples: [multiplayer.md](multiplayer.md).

### For testing and development

| Option | What it does |
|---|---|
| `--play N` | Start a new game directly on map slot N (0-based: `--play 0` is floor 1). |
| `--difficulty N` | Game type for `--play`: 0–2 difficulty 1–3 (default 1, difficulty 2), 3 Random. |
| `--sequence NAME` | Play one animated sequence or movie (`F1`–`F7`), then quit. |
| `--viewer N` | The map viewer, starting at map slot N. |
| `--shot TICS` | Headless: run TICS tics, write the frame to `--shot-out` (default `shot.png`) and exit. With `--shot-keys` (keys held, hex scan codes) and `--shot-press CODE@TIC`. `--shot-realtime` runs at 70 tics a second. |
| `--net-loss PERCENT`, `--net-place X,Y,ANGLE` | Network testing: drop outgoing packets; respawn at a fixed spot. |
| `--version`, `--help` | The version, and the full list. |

---

## The settings file

OBCR saves its settings to `obcr.toml` when you quit ([where](#where-obcr-keeps-things)). It is plain [TOML](https://toml.io); you can edit it while OBCR is not running. Missing entries take their defaults, so deleting a line, or the whole file, resets it. A fresh file looks like this (the bindings are shortened):

```toml
[video]
mode = "enhanced"        # "classic" or "enhanced"
scale = 4                # 2..8
aspect = "16:9"          # "4:3", "16:10", "16:9", "21:9"
fullscreen = false
vsync = true             # not in OPTIONS: wait for the display's refresh
smooth = false           # SMOOTH FILTER
integer_scale = false
interpolate = true       # SMOOTH MOTION
xbrz = 4                 # TEXTURES: 0 (original) or 2..6
xbrz_menus = true
xbrz_hud = false
smooth_floors = true     # FLOORS
window_width = 1280      # remembered window size
window_height = 720

[audio]
master = 0.8             # volumes 0.0..1.0
sfx = 1.0
music = 0.7
channels = 4             # SOUND VOICES: 1 or 4
effects = "digitized"    # "digitized", "adlib", "pc"
music_source = "cd"      # "cd" or "adlib"
cd_next_track = true     # CD TRACK END: true = NEXT TRACK

[input]
profile = "modern"       # CONTROL SCHEME: "modern" or "original"
mouse_sensitivity = 1.0  # MOUSE SPEED, 0.25..4.0
invert_mouse = false     # not in OPTIONS
always_run = false
gamepad_deadzone = 0.2   # not in OPTIONS: stick dead zone, 0.0..1.0
gamepad_turn_speed = 1.0 # PAD TURN SPEED, 0.25..3.0
rumble = true

[input.bindings]
forward = ["W", "UP", "DPAD UP"]
fire = ["CTRL", "MOUSE LEFT", "PAD RT"]
use = ["SPACE", "E", "MOUSE RIGHT", "PAD A"]
# ... one line per action

[game]
gametype = 1             # GAME TYPE: 0..2 difficulty 1..3, 3 Random, 4 N, 5 T, 6 M
viewsize = 18            # VIEW SIZE: 12, 14, 16, 18, 20
floor_textures = true    # F5 / F6
# quicksave_slot = 3     # the slot F9 saves to (the last one saved from the menu)
# scores = [...]         # the high-score table

[net]
port = 16387
join = ""                # the address last called
choice = 0               # character or team last chosen

# data_dir = "D:/Games/OBC/data"   # where the game files are, if not found otherwise

# [game] also keeps:
# active_mod = "lobby-assault"     # the mod chosen in MODS ("" = the original game)
# [game.mod_scores]                # each mod's high-score table
```

**Binding names.** Keys are named as on a US keyboard: letters and digits, `SPACE`, `ENTER`, `ESC`, `TAB`, `BACKSPACE`, `CTRL`, `ALT`, `LEFT SHIFT`, `RIGHT SHIFT`, `UP`, `DOWN`, `LEFT`, `RIGHT`, `INSERT`, `DELETE`, `HOME`, `END`, `PAGE UP`, `PAGE DOWN`, `F1`–`F12`, `PAUSE`, `KEYPAD +` and so on. The mouse: `MOUSE LEFT`, `MOUSE RIGHT`, `MOUSE MIDDLE`, `MOUSE 4`, `MOUSE 5`, `WHEEL UP`, `WHEEL DOWN`. The gamepad: `PAD A`, `PAD B`, `PAD X`, `PAD Y`, `PAD LB`, `PAD RB`, `PAD LT`, `PAD RT`, `PAD BACK`, `PAD START`, `PAD L3`, `PAD R3`, `DPAD UP`, `DPAD DOWN`, `DPAD LEFT`, `DPAD RIGHT`. Gamepad buttons follow the Xbox layout. On other pads they are the buttons in the same positions: A is the bottom face button, Y the top. An unknown name (a typo, say) is skipped when the file is read, and actions missing from the file take the scheme's defaults.

---

## Where OBCR keeps things

| What | Windows | Linux |
|---|---|---|
| Settings, `obcr.toml` | `%APPDATA%\OBCR\config\` | `~/.config/obcr/` |
| Saved games | `%APPDATA%\OBCR\data\saves\` | `~/.local/share/obcr/saves/` |
| A mod's saved games | `…\saves\<mod id>\` | `…/saves/<mod id>/` |
| Installed mods | `%APPDATA%\OBCR\data\mods\` | `~/.local/share/obcr/mods/` |
| An installation made by Setup | `%LOCALAPPDATA%\Programs\OBCR\` | `~/.local/opt/OBCR/` |
| The game files in it | `…\OBCR\data\` | `…/OBCR/data/` |

OBCR never writes into the game files' folder. Saved games are OBCR's own format and record the release they were made with (the floppy or the CD release): a game saved with one cannot be loaded with the other's files. The original's `CONFIG.DAT` and DOS saved games are not read.

Uninstalling with Setup keeps settings and saved games unless you ask it to remove them.
