# YAML Job File Reference

A job file defines one setup: the post-processor, clearance height, and a list of operations. Each operation specifies what to machine and how.

## Top-Level Fields

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `name` | string | no | filename stem | Program name. Used in NC preamble/postamble. |
| `postprocessor` | string | yes | | Post-processor ID (e.g. `heidenhain`, `haas`). |
| `clearance` | number | yes | | Safe Z height for rapid moves (mm or inch, matches `units`). |
| `units` | string | no | `mm` | `"mm"` or `"inch"`. |
| `operations` | list | yes | | List of operations (see below). |

## Operation Fields

### Common Fields (All Operation Types)

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `type` | string | yes | | Operation type: `"drill"`. More types planned: `"profile"`, `"pocket"`, `"facing"`. |
| `strategy` | string | yes | | Strategy for this operation type. Drill strategies: `"manual"`. Planned: `"simple"`, `"peck"`, `"bore"`, `"tap"`, `"chip_break"`. |
| `tool_number` | integer | no | 1 | Machine tool number (T word). |
| `tool_name` | string | no | `""` | Tool description. For reference only — not used in NC output by all post-processors. |
| `tool_diameter` | number | no | 0.0 | Tool diameter in working units. |
| `spindle_speed` | number | yes | | Spindle RPM (S word). |

### Drill Operation

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `points` | list of `[x, y]` | yes | | XY positions to drill. Each entry is a two-element array. |

Planned drill fields (not yet implemented):

| Field | Type | Strategy | Description |
|---|---|---|---|
| `depth` | number | simple, peck, bore, tap | Total depth below WCS zero (positive value, applied as negative Z). |
| `feed_rate` | number | simple, peck, bore, tap | Plunge feed rate. |
| `peck_depth` | number | peck | Infeed depth per peck. |
| `tap_pitch` | number | tap | Thread pitch for synchronized tapping. |

### Profile Operation (Planned)

| Field | Type | Required | Description |
|---|---|---|---|
| `color` | string | yes | SVG stroke color to select geometry (e.g. `"#ff0000"`). |
| `side` | string | yes | `"outside"`, `"inside"`, or `"on"`. |
| `feed_rate` | number | yes | XY cutting feed rate. |
| `plunge_rate` | number | yes | Z plunge feed rate. |
| `depth` | number | yes | Total depth below WCS zero. |
| `stepdown` | number | yes | Z increment per pass. |
| `compensation` | string | no | `"cam"` (default) or `"controller"` (emit G41/G42). |

### Pocket Operation (Planned)

| Field | Type | Required | Description |
|---|---|---|---|
| `color` | string | yes | SVG stroke color to select geometry. |
| `feed_rate` | number | yes | XY cutting feed rate. |
| `plunge_rate` | number | yes | Z plunge feed rate. |
| `depth` | number | yes | Total depth below WCS zero. |
| `stepdown` | number | yes | Z increment per pass. |
| `stepover` | number | no | Fraction of tool diameter (0.0-1.0). Default: 0.7. |

### Facing Operation (Planned)

| Field | Type | Required | Description |
|---|---|---|---|
| `feed_rate` | number | yes | XY cutting feed rate. |
| `plunge_rate` | number | yes | Z plunge feed rate. |
| `depth` | number | yes | Total depth below WCS zero. |
| `stepdown` | number | yes | Z increment per pass. |
| `stepover` | number | no | Fraction of tool diameter (0.0-1.0). Default: 0.7. |

## Example

```yaml
name: bracket
postprocessor: heidenhain
clearance: 10.0

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
      - [25.0, 65.0]
```

## Error Handling

Missing required fields cause a hard error to stderr with exit code 1. Chipmunk never guesses or fills in defaults for required values — fix the input.
