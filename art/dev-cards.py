# The five development card faces. One card, one drawing, one number.
#
# Run it with `python3 art/dev-cards.py` from anywhere; it writes the five
# `art/dev-*.svg` files, which are the copies the binary carries. The five share
# a template and their geometry is arithmetic, so the template is the source and
# the files are its output, rather than five drawings kept in step by hand. It
# is a tool, not part of the build: nothing in the Rust workspace runs it.
#
# Brand orange ground, white text, and the pictogram in a three-step ramp of
# white through very light warm grey, which is the same lighting the isometric
# pieces already use: lightest on top, mid on the right, shade on the left.
import os

ACCENT = '#E8542F'
LIT, MID, SHADE = '#FFFFFF', '#F6EFE8', '#E4D6C8'
INK = '#FFFFFF'

W, H = 100.0, 132.0          # a card's proportions, as the dock draws them
R = 8                        # 8% of the short side, style.md §3.1

def card(name, art, label, lines):
    """A face: ground, name, drawing, number."""
    title = ''.join(
        f'<text x="50" y="{16.5 + k * 11}" class="devName">{t}</text>'
        for k, t in enumerate(lines))
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{W:g}" height="{H:g}" '
        f'viewBox="0 0 {W:g} {H:g}">'
        # Named rather than lettered: these drawings are inlined into the page,
        # where a rule called ".n" is a rule aimed at every element on it.
        '<style>'
        '.devName{font:400 8.5px Audiowide,system-ui,sans-serif;fill:#FFFFFF;'
        'text-anchor:middle;letter-spacing:.05em;opacity:.95}'
        '.devVal{font:400 26px Audiowide,system-ui,sans-serif;fill:#FFFFFF;'
        'text-anchor:middle}'
        '.devArt{font:400 34px Audiowide,system-ui,sans-serif;fill:#FFFFFF;'
        'text-anchor:middle}'
        '</style>'
        f'<rect width="{W:g}" height="{H:g}" rx="{R}" fill="{ACCENT}"/>'
        f'{title}'
        f'{art}'
        f'<text x="50" y="120" class="devVal">{label}</text>'
        '</svg>')

def iso_card(cx, cy, w=22.0, tilt=0.0, lit=LIT, disc=ACCENT):
    """A resource card, as the game draws one: a face and a disc."""
    h = w * 1.32
    x, y = cx - w / 2, cy - h / 2
    turn = f' transform="rotate({tilt} {cx} {cy})"' if tilt else ''
    return (f'<g{turn}>'
            f'<rect x="{x:.1f}" y="{y:.1f}" width="{w:.1f}" height="{h:.1f}" '
            f'rx="{w * 0.14:.1f}" fill="{lit}"/>'
            f'<circle cx="{cx:.1f}" cy="{cy:.1f}" r="{w * 0.23:.1f}" fill="{disc}"/>'
            '</g>')

# ---- militia: the robber, the piece the card actually moves -----------------
# The geometry is art/robber.svg, refilled: its greys are a lighting ramp and
# this is the same ramp in white.
robber = (
    '<g transform="translate(34.4 36) scale(0.78)">'
    f'<polygon fill="{MID}" points="20,18 40,28 40,72 20,82 0,72 0,28"/>'
    f'<polygon fill="{LIT}" points="20,18 40,28 20,38 0,28"/>'
    f'<polygon fill="{SHADE}" points="0,28 20,38 20,82 0,72"/>'
    f'<polygon fill="{MID}" points="20,38 40,28 40,72 20,82"/>'
    f'<polygon fill="{MID}" points="20,6.5 31,12 31,28 20,33.5 9,28 9,12"/>'
    f'<polygon fill="{LIT}" points="20,6.5 31,12 20,17.5 9,12"/>'
    f'<polygon fill="{SHADE}" points="9,12 20,17.5 20,33.5 9,28"/>'
    f'<polygon fill="{MID}" points="20,17.5 31,12 31,28 20,33.5"/>'
    '</g>')

# ---- road building: two of them, the thing you get -------------------------
def road(x, y, k=0.66):
    # art/road-30.svg exactly, refilled: the board's own diagonal, running top
    # left to bottom right. Its transform is the one that drawing carries, so
    # the piece on the card is the piece on the board and not a lookalike.
    return (f'<g transform="translate({x} {y}) scale({k})">'
            '<g transform="scale(-1 1) rotate(60 -.01 -5.02)">'
            f'<path fill="{MID}" d="M0.4163,5.6005 L8.8239,0.4846 L17.2316,5.6005 '
            'L17.2316,74.3496 L8.8239,79.4655 L0.4163,74.3496 Z"/>'
            f'<polygon fill="{LIT}" points="0.4163,5.6005 8.8239,0.4846 '
            '8.8239,69.2337 0.4163,74.3496"/>'
            f'<polygon fill="{SHADE}" points="0.4163,74.3496 8.8239,69.2337 '
            '17.2316,74.3496 8.8239,79.4655"/>'
            '</g></g>')
# Offset across the road's own axis, not along it: end to end they read as
# one long road with a joint in it, which is not what "+2" means. Close enough
# that the pair reads as one mark rather than as two drawings sharing a card,
# moved symmetrically about the midpoint so the drawing stays centred.
roads = road(24, 60, 0.6) + road(34, 46, 0.6)

# ---- invention: two cards, taken from the supply ---------------------------
invention = iso_card(35, 70, 32, -9) + iso_card(66, 66, 32, 9)

# ---- monopoly: every card of one kind, gathered ----------------------------
monopoly = (iso_card(28, 72, 26, -16, MID)
            + iso_card(72, 72, 26, 16, MID)
            + iso_card(50, 68, 32, 0, LIT))

# ---- victory point: no drawing, by request ---------------------------------
vp = '<text x="50" y="80" class="devArt">VP</text>'

cards = [
    ('militia', robber, '+1', ['MILITIA']),
    ('road-building', roads, '+2', ['ROAD', 'BUILDING']),
    ('invention', invention, '+2', ['INVENTION']),
    ('monopoly', monopoly, 'ALL', ['MONOPOLY']),
    ('victory-point', vp, '+1', ['VICTORY', 'POINT']),
]
out = os.path.dirname(os.path.abspath(__file__))
for name, art, label, lines in cards:
    open(os.path.join(out, f'dev-{name}.svg'), 'w').write(card(name, art, label, lines))
    print('wrote dev-%s.svg' % name)
