# Terrain surface prompt — ORE (mountains)

Companion to `TERRAIN-PROMPT.md`. Same output rules; this one is **illustrated,
not rendered**, and that is the whole difficulty.

The reference is a printed board: flat areas of colour with hand-drawn linework
over them, in the manner of hachure on an antique topographic map. It is not a
photograph of rock, not a 3D render, not a texture map. Ask for "mountains from
above" and you will get photorealistic scree every time — the words that matter
here are *illustration*, *linework* and *flat colour*.

---

**Generate a terrain surface: MOUNTAINS — the terrain that produces ore — as
source artwork for a digital board game. Hand-illustrated topographic map art,
seen from directly overhead.**

### Style — read this first, it overrides any default

- **Flat illustration.** Areas of solid colour with drawn linework on top.
  Think mid-century screen-printed national-park poster, or the hachured relief
  of a nineteenth-century survey map.
- **Relief is drawn as line, not as light.** Ridges are described by **fine
  parallel strokes fanning down each slope**, perpendicular to the ridge crest,
  denser on the shaded flank and sparser on the lit one. The strokes are the
  shading. There is no soft gradient anywhere.
- **Absolutely not**: photorealism, photographic rock texture, 3D rendering,
  volumetric shading, ambient occlusion, specular highlight, bump or normal
  mapping, displacement, raytraced shadow, or anything that looks like a game
  engine material.
- **No outlines around forms.** Shapes are defined by colour edge and by the
  hatching itself.
- A light, even **paper grain** over the whole image. Print texture, not
  photographic noise.
- If the result could be mistaken for a photograph of a mountainside, it has
  failed regardless of how good it looks.

### Output

- PNG or JPEG, **2048 × 2048 px, square**. No transparency.
- No border, no frame, no vignette, no rounded corners. The artwork runs to the
  very edge on all four sides.
- No stray pixels, no watermark, no signature.

### What this image is

- One continuous stretch of high country seen from directly overhead, filling
  the entire square.
- It is a **sample of terrain, not a picture of a place.** Hexagons are cropped
  from it at arbitrary positions and used as game tiles.
- Therefore: **no composition.** No summit, no peak, no single dominant ridge,
  no valley through the middle. **No point in the image may be more important
  than any other.** A viewer should not be able to say where the mountain is —
  the whole square is mountain.
- Vary the terrain — branching ridgelines, broad scree fans, gravel benches —
  but nothing stands out as *the* subject.

### Colour

- A **narrow printed palette, four or five colours**, used flat:
  - cool blue-violet for shaded flanks
  - a lighter dusty periwinkle for lit flanks
  - warm bone or cream for the highest ground and gravel flats
  - a warm rust-brown for the hatching lines and exposed rock
  - optionally one deeper indigo, used sparingly for the deepest folds
- Colour may be **rich and confident**; this is printed ink, not a grey
  photograph.
- **No pure white and no black.** Bone is the lightest value, indigo the
  darkest. Nothing brighter or darker than those two.
- Lights should be **broad and soft-edged**, never small bright specks — a
  scatter of tiny high-contrast marks reads as noise under a game piece.
- **No snow, no ice, no glacier.** Bone-coloured high ground is fine; a white
  snowfield is not.
- **No saturated foreign accents** — nothing green, red, orange or turquoise
  that could be mistaken for a game piece. No lichen, no meltwater.

### Projection

- **Straight down, orthographic.** Camera directly overhead. No vanishing
  point, no convergence, no tilt, no horizon.
- A ridge shows its **top**, never its face. A visible cliff face means the
  camera has tilted and the artwork is unusable.
- Light direction is implied by hatching density alone — heavier strokes on the
  lower-right flank of every ridge — not by rendered shadow.
- The image has no "up" beyond that convention; crops are taken at any position.

### Scale

- The repeating element is the **ridge-to-ridge spacing**: **45–60 px** on the
  2048 px image, roughly **2.5% of its width**.
- Expect **30–45 distinct ridge or fan structures** across the square.
- Individual hatch strokes are fine — one to two pixels — and read as texture
  rather than as separate marks.
- If the generator will not go this fine and structures come out two or three
  times larger, send it anyway: the renderer compensates by cropping a larger
  region and scaling down. Oversized is recoverable. Photorealism is not.

### Do not include

Text, numbers, borders, frames, keylines, vignettes, edge fading, snow, ice,
glaciers, a summit or peak, isometric or three-quarter perspective, a visible
cliff face, a horizon, sky, clouds, photorealistic rock, 3D rendering,
volumetric or raytraced shading, gloss, specular highlight, emboss, pure white,
black, saturated non-stone colour, vegetation, water, or any single feature
that draws the eye.

---

## What to check when it comes back

In the order things actually go wrong:

1. **Does it look photographic or rendered?** The commonest failure by far.
   Look for soft volumetric shading and photographic grain in the rock. Reject.
2. **Is the relief drawn as lines?** If the ridges are shaded rather than
   hatched, it has drifted back to rendering. Reject.
3. **Any white or black?** Snow, ice or a blown highlight; a black fold. Reject.
4. **Can you see the side of anything?** The camera tilted. Reject.
5. **Is there a peak?** Something the eye lands on. Reject — every crop must
   work equally well, and a summit only works if the crop happens to contain it.
6. **Structures the right size?** If 2–3× too large, keep it; that is fixable
   in code.

## A consistency decision to make

The forest surface currently in the repo is **painterly and near-photographic**.
This one is **flat illustration**. They cannot share a board — one will look
like a mistake next to the other.

So this prompt is a fork, not an addition. If the illustrated look is the
direction, the forest wants regenerating in the same style: flat greens, canopy
described by stippled clumps and drawn edges rather than rendered volume, same
paper grain. Better to decide now than after four more terrains.

## Two numbers that were wrong earlier

Recorded so they are not reintroduced.

**Element size.** An early draft said 90–120 px on 2048, contradicting the tile
scale it was derived from. The correct figure is 45–60 px.

**Muted colour.** An early draft asked for muted tones, conflating a narrow
**value** range with low **saturation**. Saturation can be full.
