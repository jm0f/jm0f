# Carranta visual style

A reference for everything we draw, board, pieces, cards, interface, print.
Modelled on the mid-century national-park-poster idiom: bold flat colour,
printed rather than rendered, warm rather than neutral.

The point of writing it down is that four separate people (or four separate
sessions) should produce assets that sit together without coordination.

---

## 1. The one idea

**Ink on warm paper, not pixels on a screen.**

Everything follows from that. Nothing is pure white because paper isn't.
Nothing is pure black because ink isn't. Shading is a second flat colour, not a
gradient, a printer lays down another plate, it doesn't fade. Texture is the
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

Six hues, but the contrast does **not** come from hue, it comes from
**lightness**. Each terrain sits on its own rung, spaced roughly twelve points
of L\* apart, so the board reads as six distinct fields even in greyscale.

| Terrain | Hex | L\* | Hue | Role |
|---|---|---|---|---|
| Forest | `#1B5637` | ~32 | 150° | Darkest. Anchors the board. |
| Mountains | `#566373` | ~42 | 215°, low chroma | Neutral slate, deliberately the least saturated thing on the board. |
| Hills | `#C2492A` | ~50 | 15° | Burnt vermillion. The warm anchor. |
| Fields | `#E8A020` | ~70 | 38° | Saturated gold. |
| Pasture | `#A8C64A` | ~78 | 75° | Yellow-green. |
| Desert | `#E6D2A8` | ~86 | 42°, low chroma | Lightest. Pale sand. |

**Why this works, and why the previous set didn't.**

Forest and pasture are both green; fields and desert are both warm sand. Those
pairs cannot be separated by hue, the subject matter forbids it. They are
separated instead by **lightness and chroma**: forest is dark and saturated
where pasture is light and saturated; fields is mid and saturated where desert
is light and washed. Two axes doing the work that one axis couldn't.

The test: desaturate the board completely. If any two adjacent tiles merge, the
ladder is wrong.

### 2.3 Player colours

**The seats own the half of the wheel the land does not.**

Every chromatic terrain sits in one arc: hills 12°, fields 38°, desert 41°,
pasture 75°, forest 148°. That leaves **183°–337°**, cyan, blue, violet,
magenta, with nothing on the board in it. The four seats are spaced across it.

| Seat | Hex | Hue | L\* | Nearest terrain hue |
|---|---|---|---|---|
| 1 | `#2CA7BA` | 188° teal | 63 | 40° |
| 2 | `#3C2EB8` | 246° indigo | 30 | 98° |
| 3 | `#C065D2` | 290° purple | 57 | 82° |
| 4 | `#C1256B` | 333° rose | 44 | 39° |

**Hue is not enough on its own.** 154° of arc split four ways leaves
neighbours only ~45° apart, so the seats also stagger in lightness, 63 / 30 / 57 / 44, and the pairs that are closest in value are the ones
furthest apart in hue. Teal and purple are the weakest value pair at 1.22:1,
and they are 104° apart. Nothing is close on both axes at once.

**The arc is what lets pieces go bare.** Pieces carry no surround (§3.4), so
the only thing dividing a piece from the tile under it is the colour. That
only works because the seats and the land share no hue. Value contrast is no
help at all, every seat has some tile it is within 1.06:1 of, teal on fields
being the worst, so hue is carrying the whole separation.

Mountains at 215° sits inside the seats' arc; only its very low chroma keeps
it out of their way. Do not make it bluer.

**What this replaced, and why.** The previous set was blue `#0B72E8`, crimson
`#E01B4C`, bone `#F7F2E7`, violet `#6B3FA0`. Bone was 1° from desert and
1.33:1 against it, so it needed a surround to exist at all. Crimson was 27°
from hills, and 1.04:1 against blue in value. The new set's worst figures are
39° and 1.22:1.

**Do not try to solve this with lightness.** The ladder spans luminance 0.07
to 0.66; clearing 2:1 against every rung at once needs a colour darker than
`#0B0B0B` or brighter than white. No piece colour can be bright against
forest and dark against desert. Hue separation is the only move available.

### 2.4 Accent and interface

| Role | Hex |
|---|---|
| Primary action | `#E8542F`, vermillion, the one call-to-action colour |
| Highlight / selection | `#31AFC9`, teal |
| Positive | `#5C9E31` |
| Warning | `#F5A81C` |

One accent per screen. If two things are vermillion, neither is the action.

### 2.5 Rules that hold everywhere

- **No pure black, no pure white** (one exception, §2.1).
- **Every hue exists as three flat steps**, a tint, the identity value, a
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

The recurring container is a **lozenge**. A rectangle with fully rounded ends,
in paper white, holding one or two icons. It floats on the board with no border
and no shadow. It is how the interface speaks over the artwork without becoming
part of it.

### 3.3 Icons

**Isometric objects, in the same idiom as the pieces** (§3.4): three flat
faces, light from the upper left, no gradient within a face, no outline.

A resource is drawn as the thing itself seen as a small solid. A log, a
brick, a fleece, a sheaf, a cut stone, not as a symbol standing for it. One
hue each, taken from the terrain that produces it, in three flat steps.

- Readable at **20 px**. Below that an isometric solid loses its faces; use a
  count badge or a colour chip instead of shrinking the object further.
- Five to seven colours across the whole icon set, not per icon.

**This reverses an earlier rule** that made icons flat silhouettes while the
pieces were dimensional. The board carried two visual languages for no reason
anyone could state. Objects won because the pieces were already objects and
they are the larger commitment.

**A resource in hand is a card, not an icon on paper.** It is the terrain's
own colour with the tiles' own tooth over it (§5.1), so a card and the hex
that pays it are the same thing seen twice. That leaves nothing for a
terrain-coloured solid to sit against, so the face carries a plain disc
standing in for the drawing to come. The solids remain the rule everywhere a
resource is named rather than held.

**A hand is fanned, one card per card held**, overlapped left to right, so the
count is something you see before you read it. The fan stops at five and the
badge carries the true number: past that, looking has stopped being enough.
Only the front card shows its face; the rest are edges, since a centred disc
sliced by the card in front of it reads as a bite rather than as a card
underneath.

**A port wears the resource it deals in**, in that terrain's colour with the
board's own tooth over it, so which port it is arrives before any label does.
That frees the label to carry the rate, `2:1` or `3:1`, which is the thing a
player actually needs off it; it used to carry two letters of the resource
name, naming the port and leaving the rate unsaid. The ink flips between paper
and ink by the circle's lightness, since the five terrain colours span L\* 32
to 78 and no one ink reads on all of them.

**A port marker sits on the perpendicular bisector of the two intersections it
serves**, so its two legs are the same length. Pushing it out radially from the
middle of the board instead left one leg longer than the other wherever the
chord was not square to that direction, which was most of them.

**The fan compresses rather than growing.** A card is two thirds the height of
the dock, and the dock is only as wide as the board, so five stacks each
spreading a card's width per extra card was wider than the space at every
window size we render at. The whole fan gets a fixed budget and the cards
share it, the way a real fan of cards does.

**The card appears wherever a resource is chosen**, not only in the dock: the
trade composer carries it at reading size beside each name, so the thing you
are holding and the word for it are never two objects to translate between.

**A staged offer comes off the count.** While a trade is being composed the
hand shows what is left after it, not what the turn started with, because
planning the next move against a number already spoken for is planning against
a number that is wrong.

**The five are shown wood, brick, wool, wheat, ore.** That is not the order the
engine numbers them in, and it is not meant to be: the engine's numbering is
the wire format. The display order is sent with the payload, each entry
carrying its true index, so one order reorders the hand, the trade composer
and the discard card together and nothing downstream has to know.

### 3.4 Pieces

Chunky, solid, physically plausible. They should look like objects **placed on**
the board rather than printed into it. That separation is what lets flat
artwork and dimensional pieces share a surface.

Each piece is drawn as three flat faces: a lit top, a mid side, a shadowed end.
No gradients within a face. Light always from the upper left.

**No outline, no surround, no shadow.** A piece is separated from the board by
hue, which is what §2.3's arc buys. Two surrounds were built and both were
wrong. A box dilation left the diagonal edges thin, and an even offset union
read as a pale collar sitting between the piece and the board rather than as
part of either. If a piece is hard to see, the colour is wrong; do not put a
ring around it.

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

**Paper grain**, a fine uniform tooth over the whole surface. It is what makes
flat colour look printed rather than digital. Applied at 3–5% opacity.

**Halftone**. A visible dot screen in gradients and in large flat areas of
illustration. Dot size scales with the artwork, not the screen.

**Topographic contours**, fine concentric lines, in ink at 4% opacity, as a
background texture on bare paper areas. It is the style's signature and costs
nothing.

Never: photographic noise, film grain, scanlines, canvas weave, or anything
that implies a lens.

### 5.1 The three weights, as built

The interface reads as a game on a table, so the surfaces separate by
**material** and not only by colour. One noise, three strengths, heaviest at
the bottom of the stack:

| Surface | Weight | How |
|---|---|---|
| The table, behind everything | Substantial | `multiply`, opacity .55 |
| A card laid on it | Minimal | `multiply`, opacity .13 |
| A board tile | Most, and coarser | `soft-light`, opacity .45 |
| A piece standing on a tile | None. It is painted, see §5.2 | |

**The speckle is a warm brown, not black.** Multiplying black into the paper
pulled it grey-green: the texture was right and the colour was somebody
else's. A dark warm shadow multiplies into warm paper and leaves it warm.

**The noise keys on alpha, not on lightness.** Desaturating turbulence gives a
mid-grey with almost no variance, which is a layer that paints and cannot be
seen. The red channel becomes the alpha of a coloured speckle instead, with
the bottom of the range clipped away so what is left is fibre rather than fog.

**The tiles are a fine tooth, not a weave.** Their frequency is the highest of
the three, because the board is drawn in its own units and then scaled to fit
the panel: a low one gave visible clumps at the size a tile is actually seen
at, and the largest octave was doing most of the clumping. They are also the
one surface on `soft-light`: the terrain is saturated, and multiplying into it
would darken the palette rather than texture it.

**Baked, not filtered live.** `filter: url()` on a full-viewport element is
re-rasterized on every repaint. The page grain is a tiled background image,
rasterized once; the board's is a `<pattern>` filtered once and reused by all
nineteen tiles rather than nineteen live filters. `stitchTiles` is what lets
either repeat without a seam.

**Nothing grained twice.** The table's layer sits at a negative index, above
the body's own background and below everything in the flow, so a card is the
smooth thing laid on the rough one.

### 5.2 Paint on the pieces

Buildings and roads are the one thing on the table with a **finish**. They
carry a satin sheen, which is what lifts them off a tile whose grain runs
right up to them.

**Satin, not lacquer.** A low specular constant keeps the highlight dim and a
low exponent keeps it broad. Raising the exponent tightens it to a gloss spot,
which reads as plastic rather than as paint on wood. The constant is set well
under half: the pieces are already faceted by §3.4, so the sheen only has to
suggest a finish over lighting that is there. Turned up, the coloured pieces
went neon.

**Static, never sweeping.** A looping shine is constant motion across a board
people are trying to read. One light hangs over the table and stays there.

**The highlight is read off the piece's own silhouette**, by blurring its
alpha to round the edge over, lighting that, and clipping the result back
inside the piece so no sheen spills onto the tile. It follows whatever shape
is passed through it rather than a shape we guessed, and it is measured in the
drawing's own units, so it does not change with how large the piece is placed.

**The light agrees with the art.** Warm, from the upper left, matching the
faces §3.4 already paints lightest, so the sheen lands where the drawing says
the light is instead of fighting it.

---

## 6. Typography

Two faces, both **SIL Open Font Licence**, both served from the binary. The
licence matters as much as the design: a commercial webfont can be *used* but
not redistributed, and this page makes no external requests, so the file has to
live in the repository. Anything we cannot ship in the repo, we cannot use.

| Face | Role |
|---|---|
| **Fraunces** | Display. A variable serif with an optical-size axis and a `WONK` axis that flares its terminals. The retro park-poster quality, available as a switch rather than as a redraw. |
| **Figtree** | Everything else. Geometric-humanist, large x-height, level colour at small sizes. |

| Role | Treatment |
|---|---|
| Wordmark | Fraunces 700, `opsz 120`, `SOFT 20`, `WONK 1`, in vermillion on paper |
| Headings | Figtree 600, uppercase, 0.08em tracking, ink-soft |
| Body | Figtree 400, ink |
| Numerals | Tabular, always, quantities line up in columns |
| Labels | Figtree 600, uppercase, 0.08em tracking, ink-soft |

**Use the optical-size axis for what it is.** Fraunces at `opsz 144` is drawn
for a headline; the same glyphs at 14px come out as hairlines. Board numerals
were set that way once and the discs went weak. They are back in Figtree 700,
which is the right answer for the mark you read on every roll. Display faces
go on things that are looked at, not on things that are read at speed.

Never centre body text. Never justify. Never use more than two weights on one
surface.

---

## 7. Motion

Motion is used for **state**, never for decoration.

- A thing that is not yet placed **drifts**, 3 units, 2.2 seconds, all such
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
7. **Does it look rendered?** Soft volume, specular, ambient occlusion, all
   mean it has drifted out of the idiom.

---

## 9. Status

§2.2 and §2.3 are applied, `crates/carranta-ui/assets/index.html` carries the
terrain ladder and the seat arc, and the comments there restate the reasoning
so it survives being read without this file to hand.

Sections 4–7 are the standard for new work rather than a description of what
exists. Texture (§5) in particular is not implemented anywhere yet: the board
is flat colour with no paper grain, no halftone and no contours.
