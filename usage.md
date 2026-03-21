# Usage

CAMproject is a command-line CAM tool. You draw your part in Inkscape, assign stroke colors to indicate operations, write a YAML job file, and run one command to generate NC programs — one per tool.

---

## Example part

A 60×40×12mm aluminum clamp jaw:

- 2× Ø8.5mm through holes for M8 bolts, at X15 Y20 and X45 Y20
- 1× 30×8mm slot pocket, centered on the part, 6mm deep
- Outside profile cut through (12mm)

Two tools required:

| T# | Tool | Operations |
|----|------|------------|
| T1 | Ø8.5mm drill | Through holes |
| T2 | Ø8mm flat end mill | Slot pocket + outside profile |

No tool changer. Z=0 is set at the tool tip before each program.

---

## Step 1 — Draw in Inkscape

Create a new document sized to your part: **60×40mm**.

Set the document origin at **bottom-left** (Edit > Document Properties > set units to mm, check "origin at lower left" if available, otherwise the importer handles the Y-flip automatically).

Draw the following, assigning stroke colors as shown. Fill should be set to "none" for all objects.

| What to draw | Stroke color | Hex |
|---|---|---|
| 2 circles, Ø8.5mm, at (15,20) and (45,20) | Blue | `#0000ff` |
| 1 rectangle, 30×8mm, centered at (30,20) | Orange | `#ff8800` |
| 1 rectangle, 60×40mm, the part outline | Red | `#ff0000` |

> In Inkscape: select an object → Object > Fill and Stroke (Shift+Ctrl+F) → Stroke paint → Flat color → enter hex value.

Save as **`clamp_jaw.svg`**.

---

## Step 2 — Write the job YAML

Create **`clamp_jaw.yaml`** next to the SVG:

```yaml
postprocessor: heidenhain
clearance: 10.0          # Z height for rapid moves, relative to tool tip

operations:

  - color: "#0000ff"     # blue circles → through holes
    type: drill
    tool_number: 1
    tool_name: "Drill 8.5mm"
    tool_diameter: 8.5
    spindle_speed: 800
    feed_rate: 80
    depth: 14.0          # 2mm through, part is 12mm thick
    strategy: peck
    peck_depth: 4.0

  - color: "#ff8800"     # orange rectangle → slot pocket
    type: pocket
    tool_number: 2
    tool_name: "End Mill 8mm"
    tool_diameter: 8.0
    spindle_speed: 5000
    feed_rate: 600
    plunge_rate: 150
    depth: 6.0
    stepdown: 2.0
    stepover: 3.2        # 40% of tool diameter
    entry: helix
    helix_radius: 3.0

  - color: "#ff0000"     # red rectangle → outside profile
    type: profile
    side: outside
    tool_number: 2
    tool_name: "End Mill 8mm"
    tool_diameter: 8.0
    spindle_speed: 5000
    feed_rate: 600
    plunge_rate: 150
    depth: 12.0
    stepdown: 3.0
    allowance: 0.0
    lead_in: true
    compensation: cam    # tool radius offset computed in software
```

### Key fields

| Field | Meaning |
|-------|---------|
| `clearance` | Z height for all rapid moves. Relative to the tool tip (Z=0 at tip, no length compensation). |
| `depth` | Total cutting depth below Z=0. Not used for `strategy: manual`. |
| `stepdown` | Maximum depth per pass. |
| `allowance` | Extra material to leave. Set >0 for a roughing pass, 0 for finishing. |
| `compensation: cam` | Tool radius is offset in software. `controller` emits `RL`/`RR` instead. |
| `strategy` | Drill strategy — see table below. |

**Drill strategies:**

| `strategy` | Behaviour |
|---|---|
| `manual` | Rapid to XY, mandatory stop (M0). Operator drills by hand or with quill. No Z motion from machine. |
| `simple` | Feed to full depth in one pass, retract. |
| `peck` | Feed in increments (`peck_depth`), full retract between pecks to clear chips. Uses canned cycle if post-processor supports it. |
| `chip_break` | Feed in increments, partial retract to break chip without clearing. |
| `bore` | Feed down at boring rate, retract at same rate. |
| `tap` | Synchronised feed: `feed_rate = pitch × spindle_speed`. |

---

## Step 3 — Dry run

Always run with `--dry-run` first to verify the color grouping is correct:

```
$ camproject mill clamp_jaw.svg --params clamp_jaw.yaml --dry-run

Geometry groups found in clamp_jaw.svg:
  #0000ff   2 circles       → drill T1 (Drill 8.5mm)
  #ff8800   1 closed path   → pocket T2 (End Mill 8mm)
  #ff0000   1 closed path   → profile outside T2 (End Mill 8mm)

Output files that would be written:
  T1_DRILL_8.5MM.H     2 drill points
  T2_END_MILL_8MM.H    pocket (1 path, 3 passes) + profile (1 path, 4 passes)
```

If a color in the SVG has no matching entry in the YAML, it is listed as a warning:

```
Warning: SVG contains paths with color #888888 — no matching operation in job YAML, skipped.
```

---

## Step 4 — Generate NC files

```
$ camproject mill clamp_jaw.svg --params clamp_jaw.yaml --output-dir ./nc/

Writing nc/T1_DRILL_8.5MM.H ...   done
Writing nc/T2_END_MILL_8MM.H ...  done
```

---

## Output

### `nc/T1_DRILL_8.5MM.H`

```
BEGIN PGM T1_DRILL_8.5MM MM
BLK FORM 0.1 Z X+0.000 Y+0.000 Z-12.000
BLK FORM 0.2 X+60.000 Y+40.000 Z+0.000
TOOL CALL 1 Z S800
L Z+10.000 FMAX M3
CYCL DEF 203 PECKING ~
  Q200=2.000 ;SET-UP CLEARANCE ~
  Q201=-14.000 ;DEPTH ~
  Q206=80 ;FEED RATE FOR PLNGNG ~
  Q202=4.000 ;PLUNGING DEPTH ~
  Q210=0 ;DWELL TIME AT TOP ~
  Q203=0.000 ;SURFACE COORDINATE ~
  Q204=10.000 ;2ND SET-UP CLEARANCE ~
  Q212=0 ;DECREMENT ~
  Q213=1.000 ;DIST FOR CHIP BRKNG
L X+15.000 Y+20.000 FMAX M99
L X+45.000 Y+20.000 FMAX M99
L Z+10.000 FMAX M5
END PGM T1_DRILL_8.5MM MM
```

### `nc/T2_END_MILL_8MM.H` (excerpt)

```
BEGIN PGM T2_END_MILL_8MM MM
BLK FORM 0.1 Z X+0.000 Y+0.000 Z-12.000
BLK FORM 0.2 X+60.000 Y+40.000 Z+0.000
TOOL CALL 2 Z S5000
L Z+10.000 FMAX M3

; --- POCKET (slot 30x8mm, 6mm deep, 3 passes) ---
; pass 1, Z-2.000
L X+19.500 Y+20.000 FMAX     ; approach
L Z+1.000 F150               ; above surface
; helix entry
L X+21.000 Y+20.000 F150
CC X+19.500 Y+20.000
C X+19.500 Y+20.000 Z-2.000 DR+ F150  ; helical plunge
; contour pass 1
L X+19.500 Y+16.500 F600
L X+40.500 Y+16.500
L X+40.500 Y+23.500
L X+19.500 Y+23.500
L X+19.500 Y+16.500
; ... (inner offset passes) ...

; pass 2, Z-4.000
; ...
; pass 3, Z-6.000
; ...

; --- PROFILE (outside 60x40mm, 12mm deep, 4 passes) ---
; pass 1, Z-3.000
L X-4.000 Y-4.000 FMAX       ; approach outside part
L Z+1.000 F150
; tangent lead-in arc
CC X-4.000 Y+0.000
C X+0.000 Y+0.000 DR- F600
; profile pass 1
L X+60.000 Y+0.000 F600
L X+60.000 Y+40.000
L X+0.000 Y+40.000
L X+0.000 Y+0.000
L X-4.000 Y+0.000
; ... (passes 2, 3, 4) ...

L Z+10.000 FMAX M5
END PGM T2_END_MILL_8MM MM
```

---

## Step 5 — Machine the part

**T1 — Drill 8.5mm:**
1. Load T1, touch off Z at tool tip
2. Transfer `T1_DRILL_8.5MM.H` to the machine
3. Run program — machine pecks both holes, stops between them for chip clearing

**T2 — End mill 8mm:**
1. Load T2, touch off Z at tool tip
2. Transfer `T2_END_MILL_8MM.H` to the machine
3. Run program — machine cuts the slot pocket, then the outside profile

---

## Common patterns

### Roughing + finishing pass

Run the profile twice: once with allowance, once clean. Override the `allowance` field on the command line without modifying the YAML:

```bash
camproject mill clamp_jaw.svg --params clamp_jaw.yaml \
  --only-color "#ff0000" --allowance 0.3 --output nc/T2_PROFILE_ROUGH.H

camproject mill clamp_jaw.svg --params clamp_jaw.yaml \
  --only-color "#ff0000" --allowance 0.0 --output nc/T2_PROFILE_FINISH.H
```

### Center drilling before through drilling

Add a center drill step before the through drill. Use a separate color for center spots:

```yaml
operations:
  - color: "#00ff00"    # green → center drill (spot only)
    type: drill
    tool_number: 1
    tool_name: "Center Drill 3mm"
    tool_diameter: 3.0
    spindle_speed: 2000
    depth: 2.0
    strategy: simple

  - color: "#0000ff"    # blue → through drill
    type: drill
    tool_number: 2
    tool_name: "Drill 8.5mm"
    ...
```

Draw the hole centers twice in Inkscape: small green circles (center drill) and blue circles the actual hole diameter. They can overlap — colors keep them separate.

### Circular pocket (boss or round recess)

A circle in the SVG becomes a circular pocket when its color is mapped to `type: pocket`. The circle boundary is used as the pocket wall — the end mill fills the area inward.

```yaml
  - color: "#aa00ff"    # purple circle → circular recess
    type: pocket
    tool_number: 2
    tool_diameter: 6.0
    depth: 3.0
    stepdown: 1.5
    stepover: 2.0
    entry: helix
    helix_radius: 2.0
```

The same color can be used for a mix of circular and rectangular pockets — all closed paths (including circles) of that color are machined as pockets.

### Manual drilling (quill or hand drill)

Use `strategy: manual` when the machine positions the tool and the operator drills by hand. The program contains only XY rapid moves — no Z motion, no spindle commands, no M0 stops.

Run the program in **single block mode** on the controller. The controller stops after each positioning move; the operator drills the hole, then presses cycle start to advance.

```yaml
  - color: "#0000ff"
    type: drill
    tool_number: 1
    tool_name: "Drill 8.5mm"
    tool_diameter: 8.5
    strategy: manual        # depth, feed, spindle_speed all ignored
```

Heidenhain output:

```
TOOL CALL 1 Z S0
L Z+10.000 FMAX
L X+15.000 Y+20.000 FMAX
L X+45.000 Y+20.000 FMAX
L X+75.000 Y+20.000 FMAX
L Z+10.000 FMAX
END PGM DRILL MM
```

No `STOP` blocks, no `M3`/`M5`. The operator starts the spindle manually if needed (quill drilling), or leaves it off (hand drill). Single block mode gives full control over pacing without cluttering the program.

### Listing available post-processors

```
$ camproject postprocessors

Name              Extension
────────────────  ─────────
Heidenhain TNC    H
Haas              nc
```
