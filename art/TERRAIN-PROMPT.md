# Terrain surface prompt — WOOD (forest)

Companion to `TERRAIN-PROMPT-ore.md`. The two must look like they came off the
same press, so the shared parts below are not decoration — they are what makes
six separate images read as one set.

**Shared across every terrain:**

- The same **warm brown ink** for all linework — `#7A4A2C`, give or take.
- The same **warm cream paper** showing through — `#EFE2CC`, give or take.
- The same **paper grain** over the whole image.
- The same logic: **shading is mark density, never gradient.**
- The same element scale: the repeating feature is 45–60 px on a 2048 image.

Only the field colour and the shape of the mark change between terrains.

---

**Generate a terrain surface: FOREST — the terrain that produces wood — as
source artwork for a digital board game. Hand-engraved topographic map art,
seen from directly overhead.**

### Style — read this first, it overrides any default

- **Flat illustration, engraved.** Areas of solid colour with fine drawn
  linework over them, in the manner of a nineteenth-century survey map or a
  mid-century screen-printed park poster.
- **Canopy is described by mark, not by light.** Each tree crown is a cluster
  of short curved strokes and stipple flicks radiating from its centre —
  denser on the lower-right side of every crown, sparser on the upper-left.
  **That density difference is the entire shading.** There is no soft gradient
  anywhere in the image.
- Ink is a single warm brown; the greens sit beneath it as flat fields.
- **Absolutely not**: photorealism, photographic foliage, 3D rendering,
  volumetric shading, ambient occlusion, specular highlight, soft airbrushed
  volume, bump or normal mapping, or anything resembling a game-engine
  material.
- **No outlines around forms.** A crown is defined by where its marks stop.
- A light, even **paper grain** across the whole image — print texture, not
  photographic noise.
- If the result could be mistaken for a photograph of a forest canopy, it has
  failed regardless of how good it looks.

### Output

- PNG or JPEG, **2048 × 2048 px, square**. No transparency.
- No border, no frame, no vignette, no rounded corners. The artwork runs to the
  very edge on all four sides.
- No stray pixels, no watermark, no signature.

### What this image is

- One continuous stretch of forest canopy seen from directly overhead, filling
  the entire square.
- It is a **sample of terrain, not a picture of a place.** Hexagons are cropped
  from it at arbitrary positions and used as game tiles.
- Therefore: **no composition.** No clearing, no focal point, no path or river
  running through, nothing the eye is meant to land on. **No point in the image
  may be more important than any other.**
- Vary the packing — dense stands, thinner groves, glimpses of ground — but no
  area stands out as *the* subject.

### Colour

- A **narrow printed palette, four or five colours**, used flat:
  - a deep forest green for the shaded body of the canopy
  - a mid leaf green for the general field
  - a lighter yellow-green for scattered crowns, used sparingly
  - warm cream paper showing through as ground between the trees
  - warm brown ink for every line and stipple
- Greens may be **rich and confident** — this is printed ink, not a washed-out
  photograph.
- **No pure white and no black.** Cream is the lightest value, deep forest
  green the darkest.
- Lights are **broad and soft-edged**, never small bright specks — scattered
  high-contrast marks read as noise under a game piece.
- **No saturated foreign accents.** Nothing red, orange, blue or turquoise that
  could be mistaken for a game piece. No autumn colour, no flowers, no water.

### Projection

- **Straight down, orthographic.** Camera directly overhead. No vanishing
  point, no convergence, no tilt, no horizon.
- A tree shows its **crown**, never its trunk or its side. A visible trunk
  means the camera has tilted and the artwork is unusable.
- Light direction is implied by **stroke density alone** — heavier marks on the
  lower-right of every crown — not by rendered shadow.
- The image has no "up" beyond that convention; crops are taken at any position.

### Scale

- The repeating element is a **tree crown**: **45–60 px** across on the 2048 px
  image, roughly **2.5% of its width**.
- Expect **60–90 distinct crowns** across the square.
- Individual strokes are fine — one to two pixels — and read as texture rather
  than as separate marks.
- If the generator will not go this fine and crowns come out two or three times
  larger, send it anyway: the renderer compensates by cropping a larger region
  and scaling down. Oversized is recoverable. Photorealism is not.

### Do not include

Text, numbers, borders, frames, keylines, vignettes, edge fading, a clearing
large enough to read as one, isometric or three-quarter perspective, a visible
trunk, a horizon, sky, photorealistic foliage, 3D rendering, volumetric or
raytraced shading, gloss, specular highlight, emboss, pure white, black,
saturated non-green colour, rocks, water, flowers, or any single feature that
draws the eye.

---

## The other four terrains

Keep everything above, including the shared ink, paper, grain and scale. Change
only the field colours and the shape of the mark.

| Terrain | Field colours | The mark |
|---|---|---|
| **Hills** (brick) | Rust and ochre over cream | Short broken strokes following the contour of eroded ground, denser in the gullies |
| **Fields** (grain) | Wheat gold and straw over cream | Long parallel strokes in rows, curving with the land, boundaries where the direction changes |
| **Pasture** (wool) | Grass green, clearly distinct in hue from the forest | Fine short flicks for sward, with heavier stroke clusters marking hedgerows and lone trees |
| **Desert** | Warm sand and pale dune over cream | Long soft parallel strokes along dune crests, thinning to bare paper between them |

## What to check when it comes back

In the order things actually go wrong:

1. **Does it look photographic or rendered?** Soft volumetric shading, or
   foliage with photographic depth. The commonest failure by far. Reject.
2. **Is the shading made of marks?** If crowns are shaded rather than hatched
   and stippled, it has drifted back to rendering. Reject.
3. **Any white or black?** A blown highlight or a black gap. Reject.
4. **Any visible trunks?** The camera tilted. Reject.
5. **Is there a clearing or a path?** Something the eye lands on. Reject —
   every crop must work equally well.
6. **Crowns the right size?** If 2–3× too large, keep it; fixable in code.

## Two numbers that were wrong earlier

Recorded so they are not reintroduced.

**Element size.** An early draft said 90–120 px on 2048, contradicting the tile
scale it was derived from. The correct figure is 45–60 px.

**Muted colour.** An early draft asked for muted greens, conflating a narrow
**value** range with low **saturation**. Saturation can be full.
