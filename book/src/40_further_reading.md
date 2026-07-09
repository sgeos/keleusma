# Chapter 40. Further Reading

> This is the final chapter of the guide.

## Goal

This chapter closes the guide and points to where to go next.

## What you have done

Parts I through VIII taught the Keleusma language: its values and types,
its functions and control flow, its data shapes, the three function
categories and the host conversation, the verifier and the guarantees,
the deeper features, how a program is shipped, and the piano roll as a
working capstone. Part IX taught the other side, embedding Keleusma in a
Rust host. Between them, the guide has covered the whole of what a
Keleusma developer, on either side, needs to begin.

What follows is where to deepen that knowledge.

## A second worked example: the roguelike

The piano roll is one worked example. The repository carries a second,
larger one: a roguelike game, in `examples/rogue/`, with its long-form
manual at `docs/guide/ROGUE.md`. Where the piano roll has one `loop`
script, the roguelike is driven by a roster of scripts: a game-tick
loop, a dungeon generator, a set of monster behaviors, combat math, and
item effects. It is the example to study for how a larger application
divides its logic across many Keleusma scripts behind one Rust host.

## The reference documents

The guide explained the language. For precise lookup, the repository's
`docs/spec/` directory holds the authoritative specifications: the formal
grammar, the type system, the standard library, the instruction set, and
the wire format. The `docs/architecture/` directory holds the narrative
descriptions of the design: `LANGUAGE_DESIGN.md` for the design goals and
guarantees, and `EXECUTION_MODEL.md` for the runtime model. When a
question needs an exact answer rather than an explanation, these are the
documents to open.

## Host-side and troubleshooting references

For embedding work beyond Part IX, `docs/guide/COOKBOOK.md` collects
host-side recipes, and `docs/guide/EMBEDDING.md` is the full host-facing
reference. When the verifier rejects a program, `docs/guide/WHY_REJECTED.md`
maps the rejection messages to their causes and rewrites.
`docs/guide/FAQ.md` collects the rough edges and surprises that early
users meet. The `docs/reference/` directory holds a glossary of terms and
`RELATED_WORK.md`, which places Keleusma against the academic and
industrial work it draws on.

## The wider repository

The `examples/` directory holds smaller programs, each demonstrating one
embedding technique. The `examples/rtos/` directory holds a cooperative
real-time microkernel that runs Keleusma on embedded hardware, the
clearest demonstration of the language's intended deployment target.

## Closing

Keleusma is a small language, deliberately. It leaves out a great deal so
that it can promise a little with certainty: that a program will keep its
beat, within a known budget of time and memory, every cycle, forever.
Everything in this guide followed from that promise. A program written
inside it can be trusted in places an ordinary program cannot.

That is the end of the guide. The next step is to write something.
