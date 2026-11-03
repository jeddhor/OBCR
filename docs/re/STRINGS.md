# COUNT.EXE anchor strings

Curated strings from `COUNT.EXE` that help locate code in Ghidra (AGENT_GUIDE §5.4, recipe R3). This is an index of landmarks, not a full dump: the complete list is generated locally into `.local/ghidra/export/strings.tsv` by the `ExportAll` script and must not be committed.

- **File offset**: offset in `COUNT.EXE` (release `OBC-A`).
- **DS offset**: offset within DGROUP (file offset − 0x35580). Code that uses the string usually loads this value as an immediate (`push 0x....` / `mov ax, 0x....`) before a call. Strings below 0x35580 live in code segments and are referenced with a segment as well.
- "(+N more)": the same text occurs more than once; the first occurrence is listed.
- A leading stray letter (for example `CHIGH SCORES`, `COne moment...`) is probably a preceding byte (length, color or flag), not part of the text.

Generated 2026-11-02 from release `OBC-A`. Add rows by hand as strings become relevant; keep the columns.

### Runtime and startup

| String | File offset | DS offset |
|---|---|---|
| `Borland C++ - Copyright 1991 Borland Intl.` | 0x35584 | 0x0004 |
| `Sorry, this program requires a 286 or better.` | 0x355F4 | 0x0074 |
| `Null pointer assignment` | 0x355AF | 0x002F |
| `Floating point error: %s.` | 0x391BB | 0x3C3B |

### Program control, logging, parameters

| String | File offset | DS offset |
|---|---|---|
| `CONFIG.` | 0x3566E | 0x00EE |
| `no386` | 0x3581C | 0x029C |
| ` ANM proc ` | 0x35848 | 0x02C8 |
| ` MEM proc ` | 0x35858 | 0x02D8 |
| ` KBD proc ` | 0x35866 | 0x02E6 |
| ` PG1 proc ` | 0x35874 | 0x02F4 |
| ` PG2 proc ` | 0x35882 | 0x0302 |
| ` SND proc ` | 0x35890 | 0x0310 |
| ` CCH proc ` | 0x3589E | 0x031E |
| ` USR proc ` | 0x358AC | 0x032C |
| `cdmusic` | 0x358B9 | 0x0339 |
| `?BC exited on its own.` | 0x358C5 | 0x0345 |
| `Report written to "BC.LOG".` | 0x358FF | 0x037F |
| `BC ERROR: ` | 0x35928 | 0x03A8 |
| `BC Done.` | 0x35933 | 0x03B3 |
| `nopics` | 0x3593C | 0x03BC |
| `Dmlp exit?` | 0x35946 | 0x03C6 |
| `GOLEVEL` | 0x384A8 | 0x2F28 |
| `NOWAIT` | 0x384B0 | 0x2F30 |
| `NOCOMP` | 0x384BC | 0x2F3C |
| `HIDDENCARD` | 0x38538 | 0x2FB8 |
| `Terminal Error: %s` | 0x38510 | 0x2F90 |
| `norndseq` | 0x38A3C | 0x34BC |
| `slowrndseq` | 0x38A45 | 0x34C5 |
| `nojoys` | 0x3827E | 0x2CFE |
| `nomouse` | 0x38285 | 0x2D05 |
| `noems` | 0x38298 (+1 more) | 0x2D18 |
| `noxms` | 0x3829E (+1 more) | 0x2D1E |
| `nomain` | 0x382FB | 0x2D7B |
| `DEBUG.TXT` | 0x38240 | 0x2CC0 |

### Save, load, autosave

| String | File offset | DS offset |
|---|---|---|
| `Game Saved in AutoSave Slot.` | 0x358E0 | 0x0360 |
| `AutoSave` | 0x3598A | 0x040A |
| `svgm0.sav` | 0x35994 (+3 more) | 0x0414 |
| `autsv.sav` | 0x35A8F | 0x050F |
| `CANT CREAT SAVE FILE` | 0x35AA4 | 0x0524 |
| `CANT OPEN SAVE FILE` | 0x35ABF | 0x053F |
| `NO CURRENT SAVE` | 0x35AD3 | 0x0553 |
| `UNABLE TO SAVE` | 0x35AE3 | 0x0563 |
| `SAVED %s` | 0x35AF2 | 0x0572 |
| `CANT SAVE %s` | 0x35AFB | 0x057B |
| `No room on disk to save` | 0x35822 | 0x02A2 |

### Menus, sound, mixer

| String | File offset | DS offset |
|---|---|---|
| `BACK TO DOS ` | 0x2DA00 | (code segment) |
| `FM MUSIC VOLUME` | 0x359C6 | 0x0446 |
| `SOUND FX VOLUME` | 0x359D6 | 0x0456 |
| ` BCKGRND MUSIC ` | 0x359E6 | 0x0466 |
| ` SOUND EFFECTS ` | 0x359F6 | 0x0476 |
| ` MASTER VOLUME ` | 0x35A06 | 0x0486 |
| `MIXER NOT SUPPORTED ` | 0x35A16 | 0x0496 |
| `SOUND  %s` | 0x35A3B | 0x04BB |
| `MUSIC %s` | 0x35A4C | 0x04CC |
| `ABORT THIS GAME Y/N ` | 0x35A55 | 0x04D5 |
| `END GAME Y/N ` | 0x35A77 | 0x04F7 |
| `MEM TOO LOW ` | 0x35A6A | 0x04EA |
| `CHIGH SCORES` | 0x38827 | 0x32A7 |
| `COne moment...` | 0x35EB3 | 0x0933 |

### In-game messages, allies

| String | File offset | DS offset |
|---|---|---|
| `PAUSE` | 0x36042 | 0x0AC2 |
| `TREMORS OFF` | 0x36048 | 0x0AC8 |
| `TREMORS ON` | 0x36054 | 0x0AD4 |
| `SWITCH` | 0x3605F (+1 more) | 0x0ADF |
| `NO ALLIES!` | 0x36066 | 0x0AE6 |
| `STAYPUT & SWITCH` | 0x36071 | 0x0AF1 |
| `DISPERSE AND ATTACK` | 0x38152 | 0x2BD2 |
| `FOLLOW PLAYER` | 0x38166 | 0x2BE6 |
| `Ally stuck throttle` | 0x38174 | 0x2BF4 |
| `Ally stalled` | 0x38188 | 0x2C08 |
| `CT_Path hit a wall at %u,%u, dir %u` | 0x37F7F | 0x29FF |
| `GENERATORS ON` | 0x366E1 | 0x1161 |
| `GENERATORS OFF` | 0x366EF | 0x116F |
| `Get Ready!` | 0x3905D | 0x3ADD |

### Items and pickups

| String | File offset | DS offset |
|---|---|---|
| `BOOBY TRAP` | 0x36624 | 0x10A4 |
| `Flak Jacket` | 0x3662F | 0x10AF |
| `Increased Health` | 0x3663B | 0x10BB |
| `Grenades!` | 0x3664C | 0x10CC |
| `Galil` | 0x36656 | 0x10D6 |
| `Uzi` | 0x3665C | 0x10DC |
| `Flame Thrower` | 0x36660 | 0x10E0 |
| `Grenade Launcher` | 0x3666E | 0x10EE |
| `Map ThermoSensor` | 0x3667F | 0x10FF |
| `Fuel Pack` | 0x36690 | 0x1110 |
| `Medic Pack` | 0x366D6 | 0x1156 |
| `Fuel Bottle` | 0x366FE | 0x117E |
| ` %d Grenades` | 0x3670A | 0x118A |
| ` %d 7mm Clips` | 0x36723 | 0x11A3 |
| ` %d 9mm Clips` | 0x3673E | 0x11BE |

### Debug, cheats, diagnostics

| String | File offset | DS offset |
|---|---|---|
| `vfr750hondarc45` | 0x35A2B (+1 more) | 0x04AB |
| `nsr250` | 0x38953 | 0x33D3 |
| `president` | 0x35A85 | 0x0505 |
| `Rindex %d` | 0x36082 | 0x0B02 |
| `Findex %d` | 0x3608C | 0x0B0C |
| `Rclr %d` | 0x36096 | 0x0B16 |
| `Fclr %d` | 0x3609E | 0x0B1E |
| `Testing Log` | 0x360A6 | 0x0B26 |
| ` DIAGNOSTICS` | 0x360C2 | 0x0B42 |
| ` FLOOR: ` | 0x360CF | 0x0B4F |
| `bonuscnt<0` | 0x360D8 | 0x0B58 |
| `damagecnt<0` | 0x360E3 | 0x0B63 |
| `Total statics :` | 0x36164 | 0x0BE4 |
| `Doors         :` | 0x36186 | 0x0C06 |
| `Tot actrs  :` | 0x36197 | 0x0C17 |
| `Area DBG ON` | 0x361B2 | 0x0C32 |
| `Walk DBG ON` | 0x361CB | 0x0C4B |
| `  world X,Y: ` | 0x361E4 | 0x0C64 |
| `  tile  X,Y: ` | 0x361F5 | 0x0C75 |
| `  angle  A: ` | 0x36228 | 0x0CA8 |
| ` areanumber ` | 0x36247 | 0x0CC7 |
| ` viewheight ` | 0x36255 | 0x0CD5 |
| `WALKTHRU ON` | 0x36278 | 0x0CF8 |
| `INVINCIBLE` | 0x3626D (+1 more) | 0x0CED |
| `BOGUS MODE` | 0x3895A | 0x33DA |
| `RESTOCK` | 0x38965 | 0x33E5 |
| `HEALTH HELP` | 0x38983 | 0x3403 |
| `BEWITCHED` | 0x3899D | 0x341D |
| `UNFINISHED BUSINESS` | 0x389E8 | 0x3468 |
| `ANumSWalls %d` | 0x389DA | 0x345A |
| `Kill NULL Actor` | 0x364B2 | 0x0F32 |
| `actorat not tilemap` | 0x3669A | 0x111A |
| `pos=%d,%d map=%u segs=%u,%u act=%u a=%u` | 0x366AE | 0x112E |
| `ob->areanumber=%d, player=%d` | 0x3884C | 0x32CC |
| `BAD SWATCH` | 0x38195 | 0x2C15 |
| `frame %d uncool chunk # %d ` | 0x38A0B | 0x348B |

### Resource files and memory

| String | File offset | DS offset |
|---|---|---|
| `VGAHEAD.` | 0x381A0 | 0x2C20 |
| `VGAGRAPH.` | 0x381B4 | 0x2C34 |
| `VGADICT.` | 0x381C8 | 0x2C48 |
| `MAPHEAD.` | 0x381DC | 0x2C5C |
| `MAPTEMP.` | 0x381F0 | 0x2C70 |
| `AUDIOHED.` | 0x38204 | 0x2C84 |
| `AUDIOT.` | 0x38218 | 0x2C98 |
| `AUDIOMUS.` | 0x3822C | 0x2CAC |
| `GFXTILES.` | 0x382CC | 0x2D4C |
| `Can't open ` | 0x3824A | 0x2CCA |
| `Sorry, Not Enough Conv Mem.` | 0x382A4 | 0x2D24 |
| `EMMXXXX0` | 0x382F2 (+1 more) | 0x2D72 |
| `Null addr from GetEMSAddress` | 0x3830E | 0x2D8E |
| `Null addr from MM_GetPtr` | 0x3832B | 0x2DAB |
| `Bad Sound Num` | 0x383B6 | 0x2E36 |
| `BLASTER` | 0x383C4 | 0x2E44 |
| `.bc` | 0x38A07 | 0x3487 |

### CD audio

| String | File offset | DS offset |
|---|---|---|
| `MSCDEX not installed...???` | 0x38A50 | 0x34D0 |
| `Failed to read TOC` | 0x38A6C | 0x34EC |
| `DiskInfo.lo_tno %d ` | 0x38A80 | 0x3500 |

### Multiplayer

| String | File offset | DS offset |
|---|---|---|
| `MODEM (Hayes type)` | 0x38CAA | 0x372A |
| `SERIAL (null modem)` | 0x38CBD | 0x373D |
| `INIT STRING` | 0x38CD1 | 0x3751 |
| `BAUD RATE` | 0x38CE0 | 0x3760 |
| `ATQ0V1X4&C1&D2` | 0x38D61 | 0x37E1 |
| `MODEM DIAL BOX` | 0x38E68 | 0x38E8 |
| `HARDWARE SETUP` | 0x38D52 | 0x37D2 |
| `Team 1` | 0x38F83 | 0x3A03 |
| `Agent` | 0x38F98 | 0x3A18 |
| `Terrorist 1` | 0x38F9E | 0x3A1E |
| `Terrorist 2` | 0x38FAA | 0x3A2A |
| `CHOOSE A PLAYER` | 0x38FB6 | 0x3A36 |
| `CHOOSE A TEAM` | 0x38FC6 | 0x3A46 |
| `IPX initialization error` | 0x38FD4 | 0x3A54 |
| `Communicating on IPX socket %X` | 0x39010 | 0x3A90 |
| `PLYR ID %d IN` | 0x39068 | 0x3AE8 |
| `KILLS` | 0x3907C | 0x3AFC |
| `IPX SndPckts %s.` | 0x390A2 | 0x3B22 |
