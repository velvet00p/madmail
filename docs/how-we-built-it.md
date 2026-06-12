# How Madmail v2 Was Built

This document describes the development process used to create Madmail v2: a general three-stage method for planning and building complex software with heavy AI assistance, followed by how that method was applied in practice.

## The General Method

The process has three distinct stages. The goal is to do the hard thinking **up front** so that later implementation is fast, consistent, and low-friction.

### Stage 0: Choose the Language and Foundation

Pick the language and core tooling deliberately. For Madmail v2 the choice was **Rust** for these reasons:

- It is compiled — you get concrete build artifacts and clear feedback.
- Warnings and errors act as a tight, continuous feedback loop.
- The ecosystem has many crates for common mail-server and async tasks.
- The type system and borrow checker catch many mistakes at compile time.

### Stage 1: Design the Complete Project Structure First

This stage sets the boundaries for everything that follows.

Before writing significant code, think through the **entire project** and define what belongs where. The structure should be explicit — similar to how framework projects separate routes, models, and services.

- Talk with a strong large-context model (Google AI Studio / Gemini class) to work through the full design.
- The output of this stage is usually **one very large, precise definition file** that describes the project, its components, responsibilities, data flows, and folder/crate layout in detail. This file later becomes (or feeds directly into) the TDD.
- From that single file you can derive the exact directory structure, crate boundaries, module organization, and even the first set of implementation tickets.

Doing this work up front reduces rework later. Contributors (human or automated) have a clear place for new code.

### Stage 2: Execute with Fast, Cheap Agentic Tools

Once the structure and direction are solid, switch to fast and inexpensive agentic coding tools (Cursor in Auto mode, similar cheap/fast agents, etc.).

- Seed the agent with the big definition (often by placing the key content into `README.md` or a planning document).
- Bring reference implementations and prior versions into the repository using **git submodules** under a `context/` directory. This provides real code to study without reinventing behaviour.
- Create a `docs/` folder for project documentation and a `plans/` (or `plan/`) folder for step-by-step implementation tickets.
- Work in very small, well-scoped steps with human review gates after each meaningful piece.
- **Test aggressively.** Write as many tests as possible — unit tests, functionality tests, and smoke tests. Test fast and test stage-by-stage during development rather than waiting until the end.
- **Keep the TDD alive.** Update the Technical Design Documents regularly throughout implementation whenever new understanding or constraints appear.
- For the first plans and critical early phases, use a capable reasoning model with large context before handing detailed work to faster agents.
- Maintain a script that can rebuild a fresh `context.txt` bundle on demand, so the planner always works with the most relevant and up-to-date reference material.

The combination of upfront structure, reference code in `context/`, stepwise plans, and continuous testing keeps implementation aligned with the design.

## How This Was Applied to Madmail v2


| Stage | What Happened |
| ----- | ------------- |
| **0** | Rust was chosen as the implementation language for the reasons listed above. |
| **1** | Extensive planning sessions were run in Google AI Studio. The result was a detailed Technical Design Document (`docs/TDD/`) plus the initial breakdown of the entire project into phases. This produced the complete high-level architecture and the precise crate and module layout used today. |
| **2** | Day-to-day implementation was done in Cursor (and similar agents). Reference projects (Madmail v1, Stalwart, Delta Chat core, Iroh, WebRTC, cmdeploy, etc.) were brought in as git submodules under `context/`. Many small tickets were created under `docs/plans/` (b1–b9 + p1). A context-bundling script (`scripts/build-planning-context.sh`) feeds fresh codebase snapshots to the planner. Tickets included tests (unit + smoke + functionality) and human review. The TDD was updated during execution. Early phases used capable planning models before detailed implementation work. |

The result is the structure you see today:

- Small, reviewable implementation steps in `docs/plans/`
- Design notes in `docs/TDD/`
- Operator and contributor guides in `docs/project/`
- Reference material in `context/`
- A living `docs/` tree that explains both the product and how it was built

See the companion document [AI-assisted development](ai-assisted-development.md) for the exact tools and division of labor between human and AI.

## Key Artifacts


| Topic                                 | Location                                                                     |
| ------------------------------------- | ---------------------------------------------------------------------------- |
| Phase-by-phase implementation tickets | `[docs/plans/](plans/)` (b1–b9, p1, and others)                              |
| Technical design (TDD)                | `[docs/TDD/README.md](TDD/README.md)`                                        |
| Project architecture tour             | `[docs/project/README.md](project/README.md)`                                |
| Build, test, and deployment           | [13 — Build, test, and deploy](project/13-build-test-deploy.md)              |
| Context bundle generator              | `[scripts/build-planning-context.sh](../scripts/build-planning-context.sh)`  |
| Planning prompts                      | `[docs/prompts/](prompts/)`                                                  |
| Reference projects & submodules       | `[context/](context-references.md)` and `[external/](context-references.md)` |


Contributions, corrections, and additional narrative are very welcome via [GitHub Discussions](https://github.com/themadorg/madmail/discussions).