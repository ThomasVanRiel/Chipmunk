# Usage

This is a worked example from YAML job file to NC output.

## Write a Job File

A job file is a YAML file that defines what to machine and how. Here's a manual drilling job with three holes:

```yaml
name: drilltest
postprocessor: heidenhain
clearance: 5.0

operations:
  - type: drill
    strategy: manual
    tool_number: 1
    tool_name: "Centerdrill"
    tool_diameter: 3.0
    spindle_speed: 1200
    points:
      - [25.0, 15.0]
      - [75.0, 15.0]
      - [75.0, 65.0]
```

Save this as `drill.yaml`.

The `manual` strategy produces an XY positioning program with no Z motion — the operator enables single block mode and drills each hole by hand with the quill.

See [yaml-reference.md](yaml-reference.md) for all available fields.

## Generate NC Code

```bash
# Print to stdout
chipmunk drill.yaml

# Write to file
chipmunk drill.yaml --output DRILL.H
```

Output (Heidenhain):

```
0 BEGIN PGM drilltest MM
1 TOOL CALL 1 Z S1200
2 ; ENABLE SINGLE BLOCK MODE FOR MANUAL QUILL DRILLING
3 M0
4 L Z+5.000 FMAX
5 L X+25.000 Y+15.000 Z+5.000 FMAX
6 L X+75.000 Y+15.000 Z+5.000 FMAX
7 L X+75.000 Y+65.000 Z+5.000 FMAX
8 END PGM drilltest MM
```

The operator:
1. Loads the program on the controller
2. Enables single block mode
3. Presses cycle start — spindle turns on, tool rapids to clearance height
4. Presses cycle start for each hole — tool rapids to the XY position
5. Drills each hole manually with the quill
6. After the last hole, presses cycle start — spindle stops

## Common Patterns

```bash
# Preview output without writing a file
chipmunk job.yaml | less

# Transfer to a network-connected machine
chipmunk drill.yaml | ftp -u ftp://machine.local/programs/DRILL.H -

# Send to a serial port (older Heidenhain controls)
chipmunk job.yaml | socat - /dev/ttyUSB0,b9600,raw

# Diff against a previous version
chipmunk job.yaml | diff previous/part.H -

# Use a different post-processor
chipmunk job.yaml --postprocessor haas
```

Stdout is always the NC code. Diagnostics and errors go to stderr. This means piping and redirection work as expected.

## Minimal Job File

The smallest valid job file:

```yaml
postprocessor: heidenhain
clearance: 5.0

operations:
  - type: drill
    strategy: manual
    spindle_speed: 1200
    points:
      - [25.0, 15.0]
```

When `name` is omitted, the filename (minus extension) is used as the program name.
