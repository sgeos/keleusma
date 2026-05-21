# Documentation Strategy

> **Navigation**: [Documentation Root](./README.md)

This document describes the documentation strategy for Keleusma. It serves as a meta-prompt for AI agents and a guide for human reviewers.

---

## Purpose

This documentation system is a **High-Density Information Architecture** designed to:

1. Serve as a complete language and process specification
2. Enable efficient AI-agent navigation without context window overload
3. Support human review through logical organization
4. Function as an external memory module for AI agents

The documentation is structured as a **knowledge graph** encoded in the file system.

---

## Design Principles

### Atomic Files

Each file contains **one concept**. This keeps the Signal-to-Noise Ratio high.

When an AI agent needs to verify a specific design decision, it loads only the relevant file rather than ingesting irrelevant material.

### Hierarchical Organization

- **High-level files** provide orientation and context
- **Low-level files** provide precision and implementation detail
- **Table of Contents files** (`README.md`) exist at each directory level

This solves the Context Window vs. Precision trade-off. Agents can navigate to the precise information needed without loading the entire specification.

### Breadcrumb Navigation

Every file contains an upward navigation link to its parent table of contents:

```markdown
> **Navigation**: [Parent Section](../README.md)
```

If an agent gets lost, it can always follow links upward to reorient.

### Naming Convention

- **UPPER_SNAKE_CASE** for file names
- **Lowercase `.md`** extension
- **README.md** for table of contents files

Examples: `LANGUAGE_DESIGN.md`, `INSTRUCTION_SET.md`, `GLOSSARY.md`

---

## Directory Structure

```
docs/
├── README.md                    # Master table of contents
├── DOCUMENTATION_STRATEGY.md    # This file
│
├── guide/                       # User-facing onboarding
│   ├── README.md
│   ├── GETTING_STARTED.md
│   ├── EMBEDDING.md
│   ├── PIANO_ROLL.md
│   ├── ROGUE.md
│   ├── WHY_REJECTED.md
│   ├── FAQ.md
│   ├── COOKBOOK.md
│   └── BIG_NUMBERS.md
│
├── architecture/                # Narrative descriptions of the implemented system
│   ├── README.md
│   ├── LANGUAGE_DESIGN.md
│   ├── EXECUTION_MODEL.md
│   ├── COMPILATION_PIPELINE.md
│   └── SUB_COROUTINES.md
│
├── spec/                        # Authoritative specifications
│   ├── README.md
│   ├── GRAMMAR.md
│   ├── TYPE_SYSTEM.md
│   ├── STANDARD_LIBRARY.md
│   ├── INSTRUCTION_SET.md
│   ├── STRUCTURAL_ISA.md
│   └── WIRE_FORMAT.md
│
├── decisions/                   # Decision lifecycle
│   ├── README.md
│   ├── RESOLVED.md
│   ├── PRIORITY.md
│   └── BACKLOG.md
│
├── process/                     # Development workflow
│   ├── README.md
│   ├── PROCESS_STRATEGY.md
│   ├── COMMUNICATION.md
│   ├── GIT_STRATEGY.md
│   ├── TASKLOG.md
│   ├── PROMPT.md
│   └── REVERSE_PROMPT.md
│
├── reference/                   # Non-spec lookup material
│   ├── README.md
│   ├── GLOSSARY.md
│   └── RELATED_WORK.md
│
├── roadmap/                     # Development phases
│   ├── README.md
│   ├── V0_3_0_SELF_HOSTING.md
│   ├── V0_4_0_NATIVE_CODEGEN.md
│   └── V0_5_0_KELEUSMA_HOST.md
│
└── extras/                      # Supplementary specs for specific examples
    ├── README.md
    ├── SONG_3_SPEC.md
    ├── SONG_4_SPEC.md
    ├── SONG_5_SPEC.md
    ├── SONG_6_SPEC.md
    ├── SONG_7_SPEC.md
    ├── SONG_8_SPEC.md
    └── SONG_9_SPEC.md
```

---

## How to Read (For AI Agents)

This section is a **meta-prompt** for AI agents working with this documentation.

### Navigating the Knowledge Tree

1. **Start at `docs/README.md`** to understand available sections
2. **Read section `README.md` files** to understand what each section contains
3. **Load atomic files only when needed** for the specific task at hand
4. **Use upward navigation links** if you need to reorient

### Context Management Strategy

**Do**:
- Load the relevant section README first to understand available files
- Load only the specific atomic files needed for the current task
- Trust that related information exists in sibling files (check the table of contents if needed)

**Do not**:
- Load all documentation files at once
- Assume a single file contains all relevant information
- Ignore navigation links when exploring unfamiliar sections

### Finding Information

| If you need... | Start here |
|----------------|------------|
| First-time setup and a working example | `guide/GETTING_STARTED.md` |
| Embedding Keleusma in a Rust host | `guide/EMBEDDING.md` |
| Recipes for common embedding patterns | `guide/COOKBOOK.md` |
| A program rejected by the verifier | `guide/WHY_REJECTED.md` |
| Surprises and rough edges | `guide/FAQ.md` |
| Language overview | `architecture/LANGUAGE_DESIGN.md` |
| Execution model and two temporal domains | `architecture/EXECUTION_MODEL.md` |
| Compilation pipeline | `architecture/COMPILATION_PIPELINE.md` |
| Bytecode wire format | `spec/WIRE_FORMAT.md` |
| Sub-coroutine primitive (V0.5.0-gated) | `architecture/SUB_COROUTINES.md` |
| Formal grammar | `spec/GRAMMAR.md` |
| Type system | `spec/TYPE_SYSTEM.md` |
| Built-in functions | `spec/STANDARD_LIBRARY.md` |
| Bytecode instruction reference | `spec/INSTRUCTION_SET.md` |
| Structural ISA description | `spec/STRUCTURAL_ISA.md` |
| Terminology | `reference/GLOSSARY.md` |
| Related work and citations | `reference/RELATED_WORK.md` |
| Design decisions, resolved | `decisions/RESOLVED.md` |
| Open questions | `decisions/PRIORITY.md` |
| Deferred items | `decisions/BACKLOG.md` |
| Development process | `process/PROCESS_STRATEGY.md` |
| Communication protocol | `process/COMMUNICATION.md` |
| Current task | `process/TASKLOG.md` |
| Git workflow | `process/GIT_STRATEGY.md` |
| Development roadmap (V0.3 self-hosting, V0.4 native codegen, V0.5 Keleusma host) | `roadmap/README.md` |
| Per-song specifications for the piano-roll example | `extras/README.md` |

### Verification Pattern

When implementing a feature:

1. Read the relevant **architecture** file for design context
2. Read the relevant **spec** file for the authoritative definition
3. Check **decisions/PRIORITY.md** for open questions
4. Check **decisions/RESOLVED.md** for settled decisions
5. Consult **reference/GLOSSARY.md** for terminology

---

## How to Review (For Humans)

### Section-Based Review

Each section can be reviewed independently:

- Approve `architecture/` without re-reading `spec/`
- Review `decisions/` changes without loading `roadmap/`
- Focus on `process/` when evaluating workflow changes

### Audit Trail

- `decisions/RESOLVED.md` documents completed decisions with rationale
- `decisions/PRIORITY.md` tracks blocking questions
- `decisions/BACKLOG.md` records deferred items

---

## Maintenance Guidelines

### Adding New Concepts

1. Determine the appropriate section
2. Create a new atomic file with UPPER_SNAKE_CASE name
3. Add navigation link at the top
4. Update the section `README.md` table of contents
5. Cross-reference from related files if appropriate

### Splitting Large Files

If a file grows beyond approximately 200 to 300 lines and covers multiple concepts:

1. Identify the distinct concepts
2. Create separate atomic files for each
3. Update the parent table of contents
4. Add cross-references between related files

### Deprecating Content

1. Remove from parent table of contents first
2. Add deprecation notice to file if keeping for history
3. Or delete file entirely if no longer relevant
4. Update any cross-references in other files
