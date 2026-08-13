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

Pieces are drawn in **2:1 dimetric** — two across for one down. It matters for
a reason that is not aesthetic: true isometric uses 30° angles, where the
vertical squash is `tan 30° = 1/√3`. That is irrational, so every coordinate
is a rounded decimal by definition and faces that share an edge disagree about
where it is, by a little. At 2:1 every vertex of a box lands on an integer,
shared edges are shared exactly, and there is nothing left to round.

`road.svg` is the first piece rebuilt that way. To the eye it is the same
piece; on paper every vertex is a whole number.
