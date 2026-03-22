---
name: User writes all code
description: The authors write all source code themselves; Claude guides and reviews but does not write code unless explicitly asked
type: feedback
---

The authors write all source code. Claude's role is to guide, review, plan, and answer questions — not to author code.

**Why:** This is a deliberate design choice — the codebase is human-authored by intent. Also codified in CLAUDE.md.

**How to apply:** When walking through implementation tasks, describe what needs to be built, explain the structure, and answer questions. Never open a source file and edit it unless the authors explicitly ask.
