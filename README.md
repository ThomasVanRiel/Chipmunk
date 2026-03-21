# Chipmunk

> Working title. See `naming.md` for candidates.

Open source CAM for people who own a machine and want to cut parts — not subscribe to software.

---

## What is this?

Chipmunk takes a drawing (SVG or DXF) and a short parameter file, and produces NC programs ready to run on your machine. No cloud. No licence fee. No feature locked behind a tier.

Post-processors are Lua scripts — small, readable, and easy to extend. Heidenhain TNC is the primary built-in target; a Haas example post-processor is included to show how to add your own. If your machine speaks something else, writing a post-processor does not require touching the core.

```bash
camproject drill holes.dxf --postprocessor heidenhain --output DRILL.H
camproject mill part.svg --params job.yaml --output-dir ./nc/
```

---

## Who is it for?

- Hobby machinists and prototype builders
- Anyone who finds Fusion 360 overkill, cloud-locked, or just annoying
- People comfortable enough to edit a YAML file and run a command

You do not need a coding background. You need a drawing, a tool, and a machine.

If you run a production workshop, use professional tooling. This project is probably not for you. It lacks error checking, has no collision detection, no gouge checking, no safety interlocks, and no simulation. The NC output is what you asked for — nothing more.

**Attend your machine. At least on the first run.**

---

## The problem

Good CAM software is either:

- **Expensive** — commercial tools cost thousands per year
- **Cloud-locked** — Fusion 360 now requires a subscription and an internet connection to save your own files
- **Abandonware** — older open source tools are unmaintained or painful to set up
- **G-code only** — most open source CAM assumes LinuxCNC or Grbl; Heidenhain conversational is an afterthought or missing entirely

Chipmunk is none of those things.

---

## How it works

You start from a drawing — a DXF from a customer, an export from your CAD tool, or something drawn in Inkscape. If you use SVG, assign stroke colors to group shapes by operation — any colors you like. Then write a short YAML file mapping each color to an operation and its parameters.

```yaml
postprocessor: heidenhain
clearance: 10.0

operations:
  - color: "#0000ff"    # your choice — any hex color
    type: drill
    tool_number: 1
    tool_name: "Drill 8.5mm"
    tool_diameter: 8.5
    spindle_speed: 800
    feed_rate: 80
    depth: 14.0
    strategy: peck
    peck_depth: 4.0
```

Run one command. Get one NC file per tool, ready to transfer to the machine.

See `usage.md` for a full worked example with NC output.

---

## Current status

**Pre-implementation.** The design is complete; code is being written now.

Planned phases:

1. **Scaffolding + import** — SVG/DXF parsing, color grouping, REST API skeleton
2. **Manual drill** — rapid to XY positions, operator drills by hand in single block mode. First real hardware test.
3. **Automatic drill cycles** — peck drilling, canned cycles (CYCL DEF 203), per-tool NC file export
4. **2.5D milling** — profiles, pockets, facing from SVG color workflow

A browser frontend (geometry selection, toolpath preview) is planned but deferred — the CLI gets you a working tool first, and a visual interface will follow once the core is solid.

---

## Design principles

**Flexible tool management.** Z=0 is defined in WCS — which often coincides with the tool tip, but does not have to. Tool length compensation is supported but not required. Operations can be grouped into one program or split one file per tool, which works well when loading tools manually without an ATC.

**Trust the operator.** The software will not warn you about aggressive feeds or deep cuts. You know your machine. There is no simulation, no collision check, no gouge check — what you program is what runs.

**Post-processors are Lua scripts.** Small, readable, and easy to extend. The toolpath logic and the NC formatting are completely separate — adding support for a new controller means writing a Lua file, not modifying the core. Heidenhain TNC is the primary built-in; a Haas example is included as a starting point for other controllers.

**CLI first.** The tool works entirely from the command line. A REST API exists as a peer interface for scripting and a future browser UI, but you never need it.

---

## On LLM usage

> "I write my slop manually."

Although LLMs can be used to write parts of algorithms (e.g. fancy autocomplete), the overall structure is written manually. Tools like OpenSpec and the likes are not mature enough to produce sustainable code — the architecture, module boundaries, and core logic are thought through and typed by hand.

Documentation is a different story. LLMs are well suited to drafting, reformatting, and maintaining docs, and this project uses them for that purpose without apology.

---

## License

MIT
