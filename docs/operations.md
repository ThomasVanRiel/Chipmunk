## Operations

- Drilling
  - Drilling
  - Manual Drilling
- Milling
  - TBA

##### General signature

| parameter | optional | type |
| - | - | - |
| `type` | required | operation ID |
| `strategy` | required | strategy ID |
| `tool_number` | optional | integer |
| `tool_name` | optional | string |
| `tool_diameter` | optional | number |
| `spindle_speed` | optional | number |

### Drilling

#### General drilling

Using canned cycle if available (e.g. CYCLE200 in Heidenhain)

##### Signature

| parameter | optional | type |
| - | - | - |
| `type` | required | `drill` |
| `strategy` | required | `general` |
| `clearance_z` | required | number |
| `points` | required | list of points |

##### Example

```yaml
- type: drill
  strategy: general
  tool_number: 1
  spindle_speed: 1200
  points:
    - [25.0, 15.0]
    - [75.0, 15.0]
    - [75.0, 65.0]
```

#### Manual drilling

##### Signature

| parameter | optional | type |
| - | - | - |
| `type` | required | `drill` |
| `strategy` | required | `manual` |
| `clearance_z` | required | number |
| `points` | required | list of points |

##### Example

```yaml
- type: drill
  strategy: manual
  tool_number: 1
  spindle_speed: 1200
  points:
    - [25.0, 15.0]
    - [75.0, 15.0]
    - [75.0, 65.0]
```

### Milling

> (none yet)

## Pattern types

- List of points
- Linear Pattern
- Circular Pattern
- Rectangular Pattern
- Data Matrix
- QR Code
