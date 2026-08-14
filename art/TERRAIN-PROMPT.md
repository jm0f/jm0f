# Terrain surface prompt

Source artwork for the board's hexes. One square image per terrain; the
renderer crops hexagons out of it at different positions, so a single file
supplies every tile of that terrain and no two look alike.

Copy everything between the rules below. Swap the two terrain-specific blocks
from the table at the end for the other five.

---

**Generate a terrain surface: FOREST — the terrain that produces wood — as
source artwork for a digital board game. Aerial map view, richly rendered
painterly foliage.**

### Output

- PNG or JPEG, **2048 × 2048 px, square**. No transparency.
- No border, no frame, no vignette, no rounded corners, no matting. The
  artwork runs to the very edge on all four sides.
- No stray pixels, no watermark, no signature.

### What this image is

- One continuous stretch of forest canopy seen from directly overhead,
  filling the entire square.
- It is a **sample of terrain, not a picture of a place.** Hexagons are
  cropped from it at arbitrary positions and used as game tiles.
- Therefore: **no composition.** No horizon, no clearing, no focal point, no
  path system implying a centre, nothing the eye is meant to land on. **No
  point in the image may be more important than any other.**
- Density and canopy shape vary across the image — denser stands, thinner
  patches, glimpses of ground — but no area stands out as *the* subject.

### Colour

- Greens may be **rich and saturated**. This is a vivid, healthy forest, not
  a washed-out one. Colour is not the thing being restrained.
- What must stay narrow is the **value range**. No canopy markedly brighter
  or darker than its neighbours; no near-white highlights, no near-black
  gaps. Strong colour at even lightness.
- Three or four green values across a **tight** band, plus warm earth glimpsed
  between canopies, dark enough not to read as bright patches.
- **No saturated non-green accents.** Nothing red, orange, blue or yellow that
  could be mistaken for a game piece. No autumn colour, no flowers, no water.

### Even tone — the load-bearing rule

Game pieces in strong flat colours are placed on arbitrary parts of this image
and must stay readable against it.

- **No high-contrast objects anywhere.** No pale rocks, no bare bright soil,
  no white flowers, no fallen logs reading as strong light-on-dark shapes.
- Variation comes from **canopy shape and packing density**, never from
  brightness jumps.
- If you are adding a detail because it makes the image more interesting, it
  is probably too loud. The goal is a beautiful *surface*, not a beautiful
  *image*.

### Projection

- **Straight down, orthographic.** Camera directly overhead. No vanishing
  point, no convergence, no tilt, no horizon. Every part of the image is at
  the same scale.
- **Relief is drawn, never projected.** Light comes consistently from the
  **upper left**; every canopy casts a soft shadow to its lower right — and
  those shadows stay inside the value range above.
- **No tree may show a trunk side or lean.** If anything leans, the projection
  has slipped and the artwork is unusable.
- The image has no "up" beyond the light direction; crops are taken at any
  position.

### Scale

- **Canopy clusters 45–60 px across** on the 2048 px image — roughly **2.5%
  of its width**.
- Expect **60–90 distinct canopy clusters** visible across the square.
- This follows from how the artwork is used: a hexagon about a fifth of the
  square's width is cropped out and rendered around 170 px wide on screen.
  Larger canopies give four or five trees per tile; smaller ones dissolve into
  texture.

### Look

- Richly rendered painterly foliage. Soft airbrushed volume, visible
  leaf-cluster detail within each canopy mass.
- Canopies irregular and lumpy — never perfect circles, never triangles —
  packed at varied density with occasional smaller understory clusters.

### Do not include

Text, numbers, borders, frames, keylines, vignettes, edge fading, drop shadows
at the image edge, a clearing large enough to read as one, isometric or
three-quarter perspective, a horizon, sky, gloss, emboss, bright highlights,
deep black shadows, saturated non-green colour, rocks, water, flowers, or any
single feature that draws the eye.

---

## The other five terrains

Keep everything above. Change only the subject line, the *Colour* hues and the
*Look* block. Every terrain keeps the same rules — narrow value range, no
high-contrast objects, no saturated foreign accents, no composition — because
the same roads and buildings land on all of them.

| Terrain | Produces | Colour | Look |
|---|---|---|---|
| **Mountains** | ore | Rich blue-grey and slate, tight value band | Ridgelines and scree fields from above, relief from hachured texture. No bright snow, no black gullies |
| **Hills** | brick | Warm rust and ochre, saturated but even | Terraced ground, eroded channels, dry earth texture |
| **Fields** | grain | Full wheat gold and straw | Parallel crop rows curving across the surface, boundaries as gentle tonal shifts |
| **Pasture** | wool | Bright grass green, clearly distinct in hue from the forest | Open sward with hedgerow divisions and scattered lone canopies |
| **Desert** | — | Warm sand and dune, rich rather than pale | Long soft dune crests and sparse scrub |

Scale carries across: whatever the repeating element is — a canopy, a dune
crest, a field's width — it should be **45–60 px** on the 2048 image.

## Two failure modes to watch

**Perspective drift.** Generators reach for three-quarter view the moment they
hear "landscape". If a tree shows a trunk, or a ridge has a visible face
rather than a top, reject it.

**Composing.** They will centre something, add one striking rock, or brighten
a patch to create interest. All three are fatal here, because the crop
position is arbitrary and every square inch has to work equally well.

## Two numbers that were wrong earlier

Recorded so they are not reintroduced.

**Canopy size.** An earlier draft said 90–120 px on 2048, which contradicted
the 11–14%-of-tile figure this is derived from. The correct figure is 45–60 px.

**Muted colour.** An earlier draft asked for muted greens. That conflated two
different things: readability under game pieces needs a narrow **value** range,
not low **saturation**. Saturation can be full.
