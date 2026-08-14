# Terrain surface prompt — ORE (mountains)

Companion to `TERRAIN-PROMPT.md`, which covers forest. Same rules; the
subject, colour and surface vocabulary change.

Mountains are the hardest of the six to get right, because the two things that
make mountain photographs beautiful — snow against black rock, and a low sun
raking across a ridge — are exactly the two things this cannot have.

---

**Generate a terrain surface: MOUNTAINS — the terrain that produces ore — as
source artwork for a digital board game. Aerial map view, richly rendered
painterly rock.**

### Output

- PNG or JPEG, **2048 × 2048 px, square**. No transparency.
- No border, no frame, no vignette, no rounded corners, no matting. The
  artwork runs to the very edge on all four sides.
- No stray pixels, no watermark, no signature.

### What this image is

- One continuous stretch of high rocky terrain seen from directly overhead,
  filling the entire square.
- It is a **sample of terrain, not a picture of a place.** Hexagons are
  cropped from it at arbitrary positions and used as game tiles.
- Therefore: **no composition.** No summit, no peak, no single dominant
  ridge, no valley running through the middle, no horizon. **No point in the
  image may be more important than any other.**
- Vary the terrain across the image — broken ridgelines, scree slopes, bare
  rock benches, gravel flats — but no feature stands out as *the* subject.
  A viewer should not be able to say where the mountain is; the whole square
  is mountain.

### Colour

- Rock may be **rich and saturated** — cool blue-grey, slate, steel, with
  warmer mineral browns and a hint of violet in the shadows. This is vivid
  stone, not a grey photograph.
- What must stay narrow is the **value range**. No area markedly brighter or
  darker than its neighbours.
- **No snow.** None. Snow is near-white and breaks the value range instantly;
  it is the single most likely thing to ruin this image.
- **No black gullies or crevasses.** Shadow bottoms out at a deep slate, never
  at black.
- **No saturated non-stone accents.** Nothing red, orange, green or blue that
  could be mistaken for a game piece. No lichen patches, no alpine flowers, no
  turquoise meltwater.

### Even tone — the load-bearing rule

Game pieces in strong flat colours are placed on arbitrary parts of this image
and must stay readable against it.

- **No high-contrast objects anywhere.** No white snowfields, no black holes,
  no single pale boulder, no bright ice.
- Variation comes from **rock texture and ridge density**, never from
  brightness jumps.
- If you are adding a detail because it makes the image more dramatic, it is
  too loud. Mountains are dramatic; this surface must not be. The goal is a
  beautiful *surface*, not a beautiful *image*.

### Projection

- **Straight down, orthographic.** Camera directly overhead. No vanishing
  point, no convergence, no tilt, no horizon. Every part of the image is at
  the same scale.
- A ridge shows its **top**, not its face. If you can see the side of a cliff,
  the camera has tilted and the artwork is unusable.
- **Relief is drawn, never projected.** Light comes consistently from the
  **upper left**; each ridge carries a soft shadow along its lower-right
  flank — and that shadow stays inside the value range above.
- Diffuse light, as at midday under thin cloud. **Not** a low sun: raking
  light throws long dark shadows and blows out the lit faces, which is the
  same failure as snow.
- The image has no "up" beyond the light direction; crops are taken at any
  position.

### Scale

- The repeating element is the **ridge-to-ridge spacing**, and it should be
  **45–60 px** on the 2048 px image — roughly **2.5% of its width**.
- Expect **30–45 distinct ridge or scree structures** across the square.
- This follows from how the artwork is used: a hexagon is cropped out and
  rendered around 170 px wide on screen.
- If the generator will not go this fine and the structures come out two or
  three times larger, send it anyway — the renderer compensates by cropping a
  larger region and scaling down. Oversized is recoverable; a tilted camera or
  a snowfield is not.

### Look

- Richly rendered painterly rock. Soft airbrushed volume across ridge backs,
  fine granular texture in the scree, visible bedding and fracture lines.
- Ridgelines irregular and branching, never parallel and never radiating from
  a point. Scree fans spreading downslope between them.
- Occasional gravel flats and bare rock benches for variety in density.

### Do not include

Text, numbers, borders, frames, keylines, vignettes, edge fading, drop shadows
at the image edge, snow, ice, glaciers, a summit or peak, isometric or
three-quarter perspective, a visible cliff face, a horizon, sky, clouds, gloss,
emboss, bright highlights, black shadows, saturated non-stone colour,
vegetation, water, or any single feature that draws the eye.

---

## What to check when it comes back

In rough order of how often it goes wrong:

1. **Any white?** Snow, ice or a blown-out lit face. Reject.
2. **Any black?** A gully or crevasse bottoming out. Reject.
3. **Can you see the side of anything?** The camera tilted. Reject.
4. **Is there a peak?** Something the eye lands on. Reject — every crop has to
   work equally well, and a summit only works if the crop happens to contain it.
5. **Long shadows?** The sun is too low; ask for diffuse midday light.
6. **Structures the right size?** If they are 2–3× too large, keep it — that
   is fixable in code.

## Two numbers that were wrong earlier

Recorded so they are not reintroduced.

**Element size.** An early draft said 90–120 px on 2048, which contradicted the
tile scale it was derived from. The correct figure is 45–60 px.

**Muted colour.** An early draft asked for muted tones. That conflated two
different things: readability under game pieces needs a narrow **value** range,
not low **saturation**. Saturation can be full — and for stone it should be,
or the tile turns into a grey smear.
