# Board art

Drop SVG files here and run `./send`.

Anything in this folder is yours: the renderer draws the board from code today,
and art placed here replaces the drawn shapes rather than sitting alongside
them. Useful names, though nothing depends on them yet:

| file | what it draws |
| --- | --- |
| `hills.svg`, `forest.svg`, `pasture.svg`, `fields.svg`, `mountains.svg`, `desert.svg` | the six terrains, one hex each |
| `settlement.svg`, `city.svg`, `road.svg`, `robber.svg` | the pieces |
| `port.svg` | the marker off the coast |

Two things that save a round trip:

- **Pointy-top hexes**, matching the board as drawn. If yours are flat-top, say
  so rather than redrawing — a rotation is one line.
- **No external references.** Fonts, images and stylesheets pulled from
  elsewhere will not survive being inlined into the page, which is served by a
  process with no network access of its own.

## Projection

Pieces are drawn in **true isometric**: the two ground axes at 30° above and
below horizontal, the vertical axis straight up, so a unit cube's three edges
project to equal lengths on screen.

The seams that showed in the first drawing were not caused by those angles.
They came from each face carrying its own copy of a shared corner, rounded
separately. Every corner here is computed once and rounded once, and the faces
quote it; two faces meeting at an edge then hold identical numbers and no gap
can open between them, whatever the angle.

Four decimals is the working precision. It is not arbitrary: at three the top
face misses being a parallelogram by 0.001, and at five by 0.00001, while four
happens to close exactly for this piece. Worth re-checking when the dimensions
change rather than assumed.

The board stays flat top-down, so a piece like this does not sit on it
directly — see the note below.

## Pieces on a flat board

A flat board and a fully isometric piece cannot share a scene. What works is
extrusion: the piece's top face is drawn in the board's own plane, aligned to
the edge or intersection it occupies, with a short skirt below it for
thickness. That reads as a solid object on a flat board, and it follows the
piece to every orientation the board needs — three for roads, and any
rotation for buildings — because the top face is computed from the board's
geometry rather than drawn once and reused.

So `road.svg` is the specification: proportions, colours, and how the three
tones fall. The renderer generates the pieces from it.
