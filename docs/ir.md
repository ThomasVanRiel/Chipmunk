# Intermediate Representation `NCBlock`

## General commands

### Operation Start

`OperationStart` Parameters:

- `text`: Optional String, operation name

Implementation:

- Heidenhain: Not standard, implemented as an empty line followed by `; <operation name>`

### Operation End

`OperationEnd` Parameters:

- `text`: Optional String, operation name

Implementation:

- Heidenhain: Empty line

### Tool Change

`ToolChange` Parameters:

- `tool_number`: Optional integer, tool number in the magazine
- `spindle_speed`: float, RPM

Implementation:

- Heidenhain: `TOOL CALL 5 Z S8000`

### Comment

`Comment` Parameters:

- `text`: String

Implementation:

- Heidenhain: `; <text>`

### Stop

`Stop` — no parameters. Programmed stop, waits for operator cycle start.

Implementation:

- Heidenhain: `M0`

### Spindle On

`SpindleOn` Parameters:

- `direction`: SpindleDirection (Cw or Ccw)

Implementation:

- Heidenhain: Merged with the next motion block via `block.state`. `M3` (CW) or `M4` (CCW).

### Spindle Off

`SpindleOff` — no parameters.

Implementation:

- Heidenhain: Merged with the adjacent motion block via `block.state`. `M5`.

## Movement blocks

### Rapid move

`Rapid` Parameters:

- `x`: float, X coordinate
- `y`: float, Y coordinate
- `z`: float, Z coordinate

Implementation:

- Heidenhain: `L X+10.000 Y-20.000 Z+50.000 FMAX`

### Linear move

`Linear` Parameters:

- `x`: float, X coordinate
- `y`: float, Y coordinate
- `z`: float, Z coordinate
- `feed`: float, feed rate

Implementation:

- Heidenhain: `L X+10.000 Y-20.000 Z+5.000 F500`

### Retract

`Retract` Parameters:

- `height`: float, Z height to retract to (in WCS)

Implementation:

- Heidenhain: `L Z+50.000 FMAX`

### Retract Full

`RetractFull` — no parameters. Retract to machine Z limit in machine coordinates.

Implementation:

- Heidenhain: `L Z+0 R0 FMAX M92`

### Home

`Home` — no parameters. Full retract followed by XY home in machine coordinates.

Implementation:

- Heidenhain: `L Z+0 R0 FMAX M92` followed by `L X+0 Y+0 R0 FMAX M92`

## Canned Cycles

### Cycle Call

`CycleCall` Parameters:

- `x`: float, X position
- `y`: float, Y position
- `z`: float, Z position

Implementation:

- Heidenhain: `L X+10.000 Y+20.000 Z+5.000 FMAX M99`

### Cycle Drill

`CycleDrill` Parameters:

- `depth`: float, total drilling depth
- `surface_position`: float, Z coordinate of the surface
- `plunge_depth`: float, depth per peck
- `feed`: float, plunge feed rate
- `dwell_top`: float, dwell time at top (seconds)
- `dwell_bottom`: float, dwell time at depth (seconds)
- `clearance`: float, set-up clearance above surface
- `second_clearance`: float, second set-up clearance (retract height between pecks)
- `tip_trough`: bool, depth measured to tip (true) or trough (false)

Implementation:

- Heidenhain: `CYCL DEF 200 DRILLING` with Q-parameters Q200–Q395

## State

Each block is annotated with an `NCState` table representing the machine state at that point in the program.

`NCState` fields:

- `spindle_on`: bool
- `spindle_direction`: SpindleDirection (Cw or Ccw)
- `coolant_on`: bool
