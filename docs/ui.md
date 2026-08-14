# Carranta interface

What the screen is, and why. Decisions here came out of a design study and a
run of questions about it; the answers are recorded as decisions rather than as
options, because the point of writing them down is that the next pass does not
reopen them.

Colour, shape and motion live in `docs/style.md`. This file is about structure.

---

## 1. The idea taken from the study

**State you can read without reading.** The study replaced numbers with
objects: opponents became rows with a colour, a face, and countable cards; a
hand became cards rather than `brick 3 wood 0`; actions became a grid whose
buttons sit in the same place every turn instead of a list of sentences that
reorders under the cursor.

What was **not** taken: the dark chrome. Carranta stays ink on warm paper.
The study is a structural reference, not a palette.

---

## 2. Regions

Four, fixed. Nothing moves between them as the position changes.

```
┌──────────────────────────────────────────────────────────────┐
│  header — name · whose turn · new game                       │
├────────────┬────────────────────────────────┬────────────────┤
│  seats     │                                │   reserved     │
│   you      │                                │                │
│   ·        │            BOARD               │   (chat, when  │
│   ·        │                                │    there are   │
│   ·        │                                │    people to   │
│  ────────  │                                │    chat with)  │
│  log       │                                │                │
├────────────┴────────────────────────────────┴────────────────┤
│  dock — your hand · actions · dice                           │
└──────────────────────────────────────────────────────────────┘
```

**Left: seats, then log.** All four players in identical rows, yours among
them — comparing yourself to the table only works if everyone is measured the
same way in the same place. History runs beneath them, because a log and a
seat row answer the same question: who did what.

**Centre: the board.** Unchanged. It is also an input device — see §5.

**Right: reserved, empty for now.** It is where chat goes when there are real
players. Held open deliberately rather than reclaimed and re-cut later.

**Bottom: the dock.** Your hand and every action you take. Your eyes stay low
on the screen during your own turn.

### Scaling

One layout, scaled — desktop, laptop and tablet all get the same arrangement
at different sizes. Regions size in relative units so the whole thing shrinks
in proportion rather than reflowing at breakpoints. No separate small layout;
phones are out of scope.

---

## 3. The dock

### Your hand

Five stacks, one per resource, always in the same order, **drawn as piles
whose thickness grows with the count** and carrying a count badge. Empty types
stay in place, greyed at zero.

The depth is the point: it tells you a fat hand is coming before the
discard-on-seven rule bites, and it does it without you reading a number.
Fixed slots mean the thing you want never moves under the cursor.

Development cards sit in their own group beside the resources.

### Actions

A fixed grid: **roll, buy a development card, play one, offer a trade, trade
with the bank, end turn.**

**Building is not here.** You build by clicking a ghost piece on the board.
The grid is for actions with no place; the board is for actions with one.

A button that is unavailable **stays put and greys** — the layout is learnable
only if it never changes. **The reason appears on hover**, so a quiet dock
does not cost you the explanation.

### Dice

Two dice objects beside the actions, showing the last roll. Clicking rolls.

---

## 4. Seat rows

Every row carries, in this order: colour, name, public victory points, cards
in hand, development cards held, settlements / cities / roads remaining, and a
badge when that seat holds the longest road or the largest militia.

Pieces remaining is on the row because it is a real signal — a player down to
their last settlements is close to winning — and it is the one the raw numbers
hide.

Opponents are bots with **names and colour dots**, not seat numbers. A row that
says who did something is easier to hold in your head than one that says which
index did it, and an offer from a name reads as coming from someone.

---

## 5. Where a decision happens

The rule: **anything with a place on the board happens on the board; anything
without one happens in a card that waits for you.**

| Moment | Where |
|---|---|
| Build a road, settlement, city | Board — click the ghost |
| Setup placement | Board — click the ghost |
| Move the robber | Board — hexes light, click one |
| Choose whom to rob | Board — the seats you may rob light up |
| Discard down to seven | Card |
| Monopoly / invention resource pick | Card |
| Incoming trade offer | Card |

The card **captures the decision but does not cover the board.** It is placed
clear of the play area, because every one of these choices is one you should
make while looking at the position. A modal that hides the thing you are
deciding about is a modal that makes you guess.

---

## 6. Trading

**Composing:** click a stack in your hand to move one card into *you give*;
click it in the tray to take it back. Wanting is the same gesture against the
five resources. The offer is built out of the cards you are already looking
at, rather than in a disconnected panel of steppers.

**Receiving:** an offer arrives as a card that waits for an answer — accept,
decline, or counter. It does not sit quietly in a panel to be missed.

---

## 7. The roll

A roll is shown, not reported. The dice settle on the number, the hexes that
match light up, and **resource cards travel from those hexes to the hands that
earned them** — into your stacks, into the opponents' rows. You see who got
paid and from where without reading anything.

Bot turns play out at whatever pace the lobby was set to (§8).

---

## 8. The lobby

A **setup screen before the game**, not a row of dropdowns above it. The board
does not exist yet when you are on it.

It carries: seat count, who holds each seat, trading mode, bot pacing, and
optionally a seed. It stays configurable **while people join** — which is what
makes it a lobby rather than a settings dialog.

**Bot pacing is a lobby choice, not a global setting**: play the bots' turns
out at reading speed so you can see what they are doing, or resolve them
instantly. Both are legitimate and it depends on the mood you are in.

### What "while people join" implies

Real players are the direction, not the current state. This pass builds the
lobby's shape — seats that read *bot* or *open*, a join link, a start button —
with bots behind every seat. The server still serves one game to one browser.

Turning the open seats real means sessions with their own identity and their
own hidden hand, and pushing updates instead of polling. The engine already
keeps per-seat hidden state, so that work is the server's, not the game's, and
the interface should not have to move when it happens.

---

## 9. Components

The interface is built on **shadcn/ui's token vocabulary and component
anatomy**, ported as plain CSS. shadcn is not a package — it is source you copy
and own — so what is borrowed is the naming and the structure, not React,
Tailwind or Radix, none of which this page can run.

Tokens: `--background`, `--foreground`, `--card`, `--popover`, `--primary`,
`--secondary`, `--muted`, `--subtle`, `--destructive`, `--border`, `--input`,
`--ring`, `--radius`. Every surface, edge and ring comes from these rather than
being chosen per component.

**One deviation.** shadcn's `--accent` is a subtle hover surface; ours was
already the vermillion call to action and is load-bearing throughout
`style.md`. The hover surface is `--subtle` instead, and `--primary` points at
`--accent`.

Components: `.btn` with `-primary` / `-secondary` / `-ghost` variants and
`-sm` / `-icon` / `-block` sizes; `.panel` as the card; `.tag` as the pill
(shadcn calls it a badge, but `.badge` here is already the count on a pile);
`.separator`; a tooltip; and the decision card as a dialog.

**A button is styled by its variant and by nothing else.** If a button needs to
look different, it needs a variant, not a rule keyed to its id.

### Two accessibility rules this pass established

**Unavailable is `aria-disabled`, never the `disabled` attribute.** A disabled
button leaves the tab order and stops emitting pointer events, so its
explanation becomes unreachable by keyboard and by touch — precisely the
audience that most needs it. The click handler guards instead.

**Explanations use a real tooltip, not `title`.** `title` never appears on
touch, never on keyboard focus, and waits a second on hover. Anything carrying
`data-tip` gets one shown on hover *and* focus, dismissed on escape, flipped
when it would leave the viewport, and wired with `aria-describedby`.

The decision card is `role="dialog"` with `aria-labelledby`, takes focus when
it opens, and closes on escape. It is deliberately **not** `aria-modal`: the
board stays live behind it, and saying otherwise would misdescribe what is
reachable.

---

## 10. Art

Icons are **isometric objects**, not flat silhouettes: three flat faces, light
from the upper left, one hue each — the same idiom as the road, settlement and
city pieces already in `art/`. This is a deliberate reversal of `style.md`
§3.3, made so that the pieces and the interface finally speak one language.
§3.3 has been rewritten to match.

Needed, and not yet drawn: five resource objects (brick, wood, wool, wheat,
ore), a development-card back and five faces, the robber, and tokens for the
longest road and the largest militia. These come from the same hand as the
existing pieces; the interface is built against placeholders in the meantime
and swapping them is a file drop into `art/`.

---

## 11. Still open

- **Bank and port trades.** Rate-based against the supply rather than a
  negotiation. Currently a button in the grid; whether it shares the trade
  card's shape or gets a simpler one is undecided.
- **The sea.** The study frames the island in a ring of water tiles and puts
  ports on them. Carranta draws ports as discs on leader lines outside the
  coast. The board itself is not up for debate, but whether the frame counts
  as board was never settled.
- **Bot names.** Invented and placeholder until told otherwise.
- **The endgame.** Neither the study nor these decisions say what winning
  looks like.
