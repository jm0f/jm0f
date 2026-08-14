# Carranta visual style

A reference for everything we draw — board, pieces, cards, interface, print.
Modelled on the mid-century national-park-poster idiom: bold flat colour,
printed rather than rendered, warm rather than neutral.

The point of writing it down is that four separate people (or four separate
sessions) should produce assets that sit together without coordination.

---

## 1. The one idea

**Ink on warm paper, not pixels on a screen.**

Everything follows from that. Nothing is pure white because paper isn't.
Nothing is pure black because ink isn't. Shading is a second flat colour, not a
gradient — a printer lays down another plate, it doesn't fade. Texture is the
tooth of the stock and the dot of the screen, never photographic noise.

---

## 2. Colour

### 2.1 Ground and ink

| Role | Value | Notes |
|---|---|---|
| Paper | `#F3EDE1` | Warm bone. The default ground for every surface. |
| Paper, raised | `#FBF7EF` | Cards and panels sitting on the paper. |
| Ink | `#33261B` | Warm near-black. All text, all linework. Never `#000`. |
| Ink, soft | `#6B5B4C` | Secondary text, rules, captions. |

Pure white appears **once**: inside the number discs on the board, where a
small hard-edged shape needs to punch through terrain. Nowhere else.

### 2.2 The terrain ladder

Six hues, but the contrast does **not** come from hue — it comes from
**lightness**. Each terrain sits on its own rung, spaced roughly twelve points
of L\* apart, so the board reads as six distinct fields even in greyscale.

| Terrain | Hex | L\* | Hue | Role |
|---|---|---|---|---|
| Forest | `#1B5637` | ~32 | 150° | Darkest. Anchors the board. |
| Mountains | `#566373` | ~42 | 215°, low chroma | Neutral slate — deliberately the least saturated thing on the board. |
| Hills | `#C2492A` | ~50 | 15° | Burnt vermillion. The warm anchor. |
| Fields | `#E8A020` | ~70 | 38° | Saturated gold. |
| Pasture | `#A8C64A` | ~78 | 75° | Yellow-green. |
| Desert | `#E6D2A8` | ~86 | 42°, low chroma | Lightest. Pale sand. |

**Why this works, and why the previous set didn't.**

Forest and pasture are both green; fields and desert are both warm sand. Those
pairs cannot be separated by hue — the subject matter forbids it. They are
separated instead by **lightness and chroma**: forest is dark and saturated
where pasture is light and saturated; fields is mid and saturated where desert
is light and washed. Two axes doing the work that one axis couldn't.

The test: desaturate the board completely. If any two adjacent tiles merge, the
ladder is wrong.

### 2.3 Player colours

Chosen against the terrain ladder, not in isolation. Each must be legible on
all six terrains and distinguishable from the other three at a glance.

| Seat | Hex | Why |
|---|---|---|
| 1 | `#0B72E8` | Saturated blue. No terrain is saturated blue — mountains are deliberately low-chroma slate to leave this space free. |
| 2 | `#E01B4C` | Crimson. 30° from hills and far higher chroma, so the two never merge. |
| 3 | `#F7F2E7` | Bone white. Reads on the four darker terrains; the surround carries it on the two lightest. |
| 4 | `#6B3FA0` | Violet. The one hue nothing else on the board occupies. |

**Mountains being low-chroma is load-bearing.** It is the concession that buys
a saturated blue player colour. Do not "improve" it by making the mountains
more blue.

### 2.4 Accent and interface

| Role | Hex |
|---|---|
| Primary action | `#E8542F` — vermillion, the one call-to-action colour |
| Highlight / selection | `#31AFC9` — teal |
| Positive | `#5C9E31` |
| Warning | `#F5A81C` |

One accent per screen. If two things are vermillion, neither is the action.

### 2.5 Rules that hold everywhere

- **No pure black, no pure white** (one exception, §2.1).
- **Every hue exists as three flat steps** — a tint, the identity value, a
  shade. Shading picks a step; it never interpolates.
- **Five to seven colours in any single illustration.** Restraint is the style.
- Adjacent things differ in **lightness**, not only hue.

---

## 3. Shape

### 3.1 Corners

Everything is rounded, and the radius is proportional, not fixed:
**radius = 8% of the shape's short side.** A card, a badge and a button
therefore look like members of one family at any size.

Two exceptions: the hexagons of the board are sharp, and the number discs are
true circles.

### 3.2 The badge

The recurring container is a **lozenge** — a rectangle with fully rounded ends,
in paper white, holding one or two icons. It floats on the board with no border
and no shadow. It is how the interface speaks over the artwork without becoming
part of it.

### 3.3 Icons

- **Flat filled silhouettes.** No outlines, no strokes, no detail lines.
- Built from primitives: a mountain is two triangles, a tree is a stacked
  triangle, water is a teardrop, sun is an eight-point star, wood is a log
  end-on.
- Readable at **16 px**. If it needs more, it is too complicated.
- One colour each, taken from the terrain ladder — the icon for a resource is
  the colour of the terrain that produces it.

### 3.4 Pieces

Chunky, solid, physically plausible. They should look like objects **placed on**
the board rather than printed into it — that separation is what lets flat
artwork and dimensional pieces share a surface.

Each piece is drawn as three flat faces: a lit top, a mid side, a shadowed end.
No gradients within a face. Light always from the upper left.

Every piece carries a **surround in the board's background colour**, 1.6 board
units wide, so its silhouette survives against any terrain. Grown as an offset
union, not a box dilation — see `crates/carranta-ui/assets/index.html`.

---

## 4. Shading and depth

**Depth is layering, not lighting.**

- Build a scene from **flat planes** stacked front to back, each one flat
  colour. A mountain range is four overlapping shapes, not one shape with
  modelling on it.
- Shadow is a **shape**, filled with the next step down the same hue's ladder.
  Never a blur, never an opacity, never a gradient.
- **One light direction: upper left.** Every shadow falls lower-right. This is
  the only 3D cue the style permits.
- **No ambient occlusion, no specular, no bevel, no emboss, no drop shadow**
  on artwork. Interface panels may carry one soft shadow to lift them off the
  paper; artwork never does.

Gradients are allowed in exactly one place: **card art skies**, as a two-stop
ramp between adjacent hues, always carrying a halftone dot overlay so it reads
as printing rather than as rendering.

---

## 5. Texture

Two textures, both subtle, both everywhere:

**Paper grain** — a fine uniform tooth over the whole surface. It is what makes
flat colour look printed rather than digital. Applied at 3–5% opacity.

**Halftone** — a visible dot screen in gradients and in large flat areas of
illustration. Dot size scales with the artwork, not the screen.

**Topographic contours** — fine concentric lines, in ink at 4% opacity, as a
background texture on bare paper areas. It is the style's signature and costs
nothing.

Never: photographic noise, film grain, scanlines, canvas weave, or anything
that implies a lens.

---

## 6. Typography

| Role | Treatment |
|---|---|
| Display | Heavy rounded geometric sans, tight tracking, often in vermillion on paper |
| Headings | Condensed geometric sans, small caps, generous letter-spacing |
| Body | Humanist sans, normal weight, ink-soft |
| Numerals | Tabular, always — quantities line up in columns |
| Labels | Small caps, 0.08em tracking, ink-soft |

Never centre body text. Never justify. Never use more than two weights on one
surface.

---

## 7. Motion

Motion is used for **state**, never for decoration.

- A thing that is not yet placed **drifts** — 3 units, 2.2 seconds, all such
  things in step so they read as one condition of the board rather than as
  separate events.
- A thing being pointed at **holds still**, so it can be clicked.
- Transitions are 120–180 ms, ease-out. Anything slower feels broken.
- All of it respects `prefers-reduced-motion`.

---

## 8. Checking a new asset

In order, because the early ones are cheap:

1. **Desaturate it.** Do adjacent areas still separate? If not, the lightness
   ladder has been violated.
2. **Is there pure black or pure white?** Almost always a mistake.
3. **Is any shading a gradient?** It should be a second flat colour.
4. **Does light fall from the upper left everywhere?**
5. **More than seven colours?** Cut.
6. **Does an icon survive at 16 px?**
7. **Does it look rendered?** Soft volume, specular, ambient occlusion — all
   mean it has drifted out of the idiom.

---

## 9. What this replaces

The flat terrain palette currently in `crates/carranta-ui/assets/index.html`
was picked by eye from a screenshot and has two known faults: desert and fields
are close in both hue and lightness, and the mountains slate is saturated
enough to crowd a blue player colour. The ladder in §2.2 fixes both. It is a
proposal until applied — the code has not been changed.
