# Icon Generation Prompt

> **Navigation**: [Extras](./README.md) | [Documentation Root](../README.md)

This document records the recommended image-generation prompt for
the Keleusma project icon, together with the design rationale and
the practical caveats that apply when regenerating it. The current
icon lives at [`assets/Keleusma_Icon.png`](../../assets/Keleusma_Icon.png)
and is displayed on the top-level [`README.md`](../../README.md).

## Concept

The mark depicts two stylized suns, rendered as a cosmic binary
pair, whose combined form suggests a horizontal infinity symbol.
The left sun is warm red and represents the host environment,
which is the unpredictable real world that drives the runtime
through registered native functions. The right sun is cool blue
and represents the deterministic Keleusma virtual machine. A
continuous figure-eight ribbon weaves between the two suns and
stands for the real-time control loop that carries execution back
and forth across the host-to-virtual-machine boundary through
typed yield and resume. The rhythmic exchange of command across
that boundary echoes the etymology of the name, which derives from
the rhythmic call a rowing master used to coordinate oar strokes.

## Branding constraints

The artwork uses light-colored fills with thick black outlines.
The intent is a light mark that remains legible on light
backgrounds, where the black outline supplies the needed
definition, and that also stands on its own on dark backgrounds,
where the light fills carry themselves and the outline simply
disappears without loss. The one condition this strategy imposes
is that black must be used only as a redundant outline around a
light-colored fill and never as the sole carrier of a shape, so
that every element survives when the outline vanishes on a dark
background. The infinity ribbon in particular must be a
light-filled band rather than black-only linework.

## Recommended prompt

```
Flat vector emblem in a bold sticker style, thick uniform black outlines and flat cel-shaded fills, cosmic theme. Two stylized suns side by side, slightly overlapping, their combined form suggesting a horizontal infinity symbol. The left sun is a warm red and orange star with golden-yellow flame points and solid golden-yellow double-headed arrows radiating outward in a full ring. The right sun is a cool blue star with pale cyan flame points and solid white double-headed arrows radiating outward in a full ring. Each sun carries a simple round clock face with two hands set to ten past ten and four bold tick marks at the top, bottom, left and right, no numerals, no text. A single continuous figure-eight ribbon weaves between and around the two suns and forms one clear unbroken infinity loop; the left half of the ribbon is golden-yellow, the right half is white, the whole ribbon edged with a thick black outline. A bright white multi-pointed sparkle marks the single center point where the ribbon crosses. Symmetrical and balanced composition, clean crisp edges, high contrast, light-colored fills intended to read against a light background, isolated on a solid flat black background, centered with even margins, square 1:1 framing.
```

## Rationale for the prompt choices

The prompt front-loads the subject and leans on the bold outline
and flat fill style that diffusion models render reliably. Several
choices target specific failure modes observed in earlier
generations.

- **Tick marks rather than numerals.** Image models render
  specified glyphs unreliably, so the prompt asks for tick marks
  and two clock hands rather than Roman numerals. Crisp numerals,
  if wanted, should be added by hand in a vector editor afterward.
- **One continuous filled ribbon rather than two direction-routed
  bands.** Earlier prompts that specified an upper-versus-lower
  color routing did not render as a continuous loop. Asking for a
  single unbroken figure-eight with a left half and a right half is
  a topology the model approximates more reliably, and a filled
  ribbon satisfies the requirement that the loop survive on dark
  backgrounds.
- **Flat outlined sticker style rather than an intricate diagram.**
  Flat outlined artwork scales toward a small icon and produces
  cleaner edges while remaining cosmic.
- **Blue and cyan core with white rays rather than an all-white
  right side.** Pure white on the right washed out and unbalanced
  the composition against the punchy red left.
- **Square framing.** Square framing future-proofs the mark for
  icon and avatar slots.

## Optional deltas

- **If the loop still fails to read,** drop the clock faces from
  the prompt and generate only the two suns plus the ribbon. Fewer
  competing structures gives the model a better chance at a clean
  loop. The faces can be composited back in.
- **For a derived small icon,** run a stripped variant that asks
  only for the two-color figure-eight ribbon with two small suns at
  the loops, no rays and no faces, square framing. This yields a
  candidate silhouette consistent with the large emblem.

## Practical notes

Expect to generate many candidates and select the one with the
cleanest loop and the most balanced symmetry. Plan to finish the
numerals, and probably to true up the symmetry and the loop
crossing, by hand in a vector tool. If clean background removal
matters and the outline is black, consider generating on a flat
neutral grey field rather than pure black, so that keying out the
background does not also remove the black outline.

## Status and provenance

The current icon was generated from an earlier and simpler prompt
and then edited by hand to convert most of the black background to
transparency while retaining a black outline around the artwork. A
dedicated small icon is not yet produced. The prompt recorded here
is a recommended starting point for regeneration and has not been
validated against a specific image generator.
