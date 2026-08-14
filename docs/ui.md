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
│  header, name · whose turn · new game                       │
├────────────┬────────────────────────────────┬────────────────┤
│  seats     │                                │   reserved     │
│   you      │                                │                │
│   ·        │            BOARD               │   (chat, when  │
│   ·        │                                │    there are   │
│   ·        │                                │    people to   │
│  ────────  │                                │    chat with)  │
│  log       │                                │                │
├────────────┴────────────────────────────────┴────────────────┤
│  dock, your hand · actions · dice                           │
└──────────────────────────────────────────────────────────────┘
```

**Left: the clock, the seats, then the log.** All four players in identical rows, yours among
them, comparing yourself to the table only works if everyone is measured the
same way in the same place. History runs beneath them, because a log and a
seat row answer the same question: who did what.

**Centre: the board.** Unchanged. It is also an input device, see §5.

**Right: reserved, empty for now.** It is where chat goes when there are real
players. Held open deliberately rather than reclaimed and re-cut later.

**Bottom: the dock.** Your hand and every action you take. Your eyes stay low
on the screen during your own turn.

### Scaling

One layout, scaled, desktop, laptop and tablet all get the same arrangement
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

A button that is unavailable **stays put and greys**, the layout is learnable
only if it never changes. **The reason appears on hover**, so a quiet dock
does not cost you the explanation.

### Dice

Two dice objects beside the actions, showing the last roll. Clicking rolls.

---

## 4. Seat rows

Every row carries, in this order: colour, name, public victory points, cards
in hand, development cards held, settlements / cities / roads remaining, and a
badge when that seat holds the longest road or the largest militia.

Pieces remaining is on the row because it is a real signal. A player down to
their last settlements is close to winning, and it is the one the raw numbers
hide.

### The log is grouped by turn

Each line carries the turn it happened in and the seat that caused it, so the
page groups and colours rather than parsing sentences back apart. A turn is a
sticky heading with the seat's name at one end and a rule in that seat's colour
under it; its lines hang off a bar in the same colour, so a run of moves by one
player reads as one block. Things the table did rather than a player, the deal
and the result, carry no colour and sit apart.

**A turn is a box**, bordered in that seat's colour on the left edge, the way
the players list boxes whoever is on the move. A turn is a unit of the game and
gets a container rather than a rule and some indentation. A turn that logged
nothing gets no box.

**No board indices.** A button has to tell two otherwise identical choices
apart and so carries the vertex or edge; the log does not. The board already
shows where the road went, and "Build road at 68" asks the reader to hold a
number that means nothing to them.

**Scrolling up stays up.** The log followed its own tail on every poll, so
reading anything older than three seconds was impossible. It only follows the
tail if you were already at it.

### The deal is made of turns too

Each player takes a turn placing, and treating those as something other than
turns collapsed eight of them into one undifferentiated block with no way to
say whose placement was whose.

They are counted in **one run with play**: four players place eight times
between them, so the first turn of play is turn 9. Three players place six
times, so theirs is turn 7.

**A new turn is not only a new decider.** The deal is a snake, so the player
who places last in the first round places first in the second, and then moves
first in play. The turn changes hands twice without the decider changing, and
both were missed: the fold merged two placement turns into one, and the last
placement ran into the first turn of play on the same clock. Entering
`PreRoll` marks the start of a turn of play and entering `SetupSettlement` the
start of a placement, each exactly once per turn.

**A line is stamped before its action is applied.** Applying moves the phase
on, and the last placement moves it out of the deal entirely, so stamping
afterwards filed that placement under a turn that had not started.

Opponents are bots with **names and colour dots**, not seat numbers. A row that
says who did something is easier to hold in your head than one that says which
index did it, and an offer from a name reads as coming from someone. The names
are applied in the page, not the engine. The log arrives saying "Seat 2" and
is rewritten, so the history and the rows never disagree.

### The two victory-point numbers are not the same measurement

**An opponent's score is public points only.** A victory-point card sitting in
a hand counts for nothing anyone else can see, and the engine withholds it
until the game ends, `apparent_vp` is `public_victory_points` during play and
the true total only at game over. **Your own row shows your real total**,
hidden cards included, because you are allowed to know your own hand.

So the column does not compare like with like, and someone on 7 showing may
already have won. The row says so rather than letting you assume otherwise:
every score carries the explanation, and your own says how much of it the table
can actually see when the two differ.

---

## 5. Where a decision happens

The rule: **anything with a place on the board happens on the board; anything
without one happens in a card that waits for you.**

| Moment | Where |
|---|---|
| Build a road, settlement, city | Board, click the ghost |
| Setup placement | Board, click the ghost |
| Move the robber | Board, hexes light, click one |
| Choose whom to rob | Board. The seats you may rob light up |
| Discard down to seven | Card |
| Monopoly / invention resource pick | Card |
| Incoming trade offer | Card |

The card **captures the decision but does not cover the board.** It takes over
the column already held for chat, because every one of these choices is one you
should make while looking at the position.

**That column is a fixed width and never changes.** An earlier version widened
it while a card was open, which shifted the board and everything on it. A panel
that resizes what the player is reading is worse than one that covers a corner:
nothing should move because a question arrived.

---

## 6. Trading

**Composing:** click a stack in your hand to move one card into *you give*;
click it in the tray to take it back. Wanting is the same gesture against the
five resources. The offer is built out of the cards you are already looking
at, rather than in a disconnected panel of steppers.

**Receiving:** an offer arrives as a card that waits for an answer, accept,
decline, or counter. It does not sit quietly in a panel to be missed.

An offer reads **"Ines offers 1 wood for 2 wheat"**. It used to be "Ines: 1
wood for 2 wheat", which left the reader to work out which side was which.

Each offer carries **the waiting loader in the proposer's colour**, beside their
name. It is the same animation an empty seat uses, and for the same reason:
somebody is waiting on an answer. The motion walks the eye from who asked to
the buttons that answer them.

**Silence is a refusal.** An offer left on the table stops the bots, because
they are waiting on you. That wait is charged to whoever owes the answer, not
to whoever holds the turn, and when their clock runs out the offers are
declined for them.

---

## 7. The roll

A roll is shown, not reported. The dice settle on the number, the hexes that
match light up, and **resource cards travel from those hexes to the hands that
earned them**, into your stacks, into the opponents' rows. You see who got
paid and from where without reading anything.

Bot turns play out at whatever pace the lobby was set to (§8).

---

## 8. The lobby

A **setup screen before the game**, not a row of dropdowns above it. The board
does not exist yet when you are on it.

It carries: your name, the size of the table, who holds each seat, an invite
link, the turn clock, and a seed. It stays configurable **while people join**, which is what makes it a lobby rather than a settings dialog.

A fresh server opens on the lobby. A reload part-way through a game does not:
the board comes back instead, because the clock is running and dealing again
would be the wrong default.

**Seats wait for people by default.** A seat can be set to a bot, but the
default is open, because the reason a lobby exists is that someone else is
coming. The server cannot seat a person yet, so an open seat is played by a bot
until it can, said plainly on the screen rather than implied.

**Your name is editable because nobody is signed in.** It is kept in the
browser between games, which is the closest thing to being remembered without
an account. When there are accounts the name comes from one and the field goes
away; the server already stores it per session rather than the page holding it,
so that swap does not move anything.

**The board cannot be dealt with an empty seat.** Dealing would quietly turn a
waiting seat into a bot, which is a decision about who is playing being made by
a mis-click. The button says how many seats are short and refuses until they
are filled or the table drops to three.

**The seed is generated, not blank.** It exists before the game does, so it can
be copied, shared or written down first.

It is written as base 36 in three groups, `0n6pc-0cu9-x0n2`, rather than as
twenty decimal digits. Twenty digits cannot be checked by eye or read down a
phone. Thirteen characters is what a u64 takes in base 36, and the engine seeds
from a u64, so padding it out to look longer would claim entropy that is not
there. Reading one back ignores case and hyphens.

**There is no market setting.** People trade freely or it is not the same game.
The restricted and closed markets remain in the engine and in its tests; they
are not something to put in front of a table of humans.

### The clock is per turn

Not a countdown on the whole game. An allowance for **thinking**, which is the
thing that actually runs long.

- **Per turn**, default **60 seconds**: a fresh allowance every turn.
- **Chess clock**: one bank each for the whole game, draining only while it is
  your move. Spend it and your turns end as soon as they begin.
- **No clock.**

It belongs to the server: `Session` holds the allowance, when the current turn
began, and what each seat has spent. Reloading the page hands nobody more time.
Each seat's own clock rides on its row in the left column; the dock shows
yours.

**Running out ends your turn, never the game.**

### What a clock is allowed to do for you

Most of the game can be declined: you can always simply end your turn. Some of
it cannot, and those are the places a clock has to act or the game stops
forever on whoever walked away.

The list is fixed by the engine's own `Phase`, not guessed at. Every phase a
player can be the decider in:

| Phase | Can it be passed? | What the clock does |
|---|---|---|
| `SetupSettlement` | No | Places at random among the legal spots |
| `SetupRoad` | No | Places at random among the legal spots |
| `PreRoll` | No, but only one move exists | Rolls |
| `Discard` | No | Discards at random down to the limit |
| `MoveRobber` | No | Moves the robber and picks the victim at random |
| `Action` | Yes | Ends the turn |
| `GameOver` | Nothing to do | Nothing |

So the ordering is: **end the turn if you can, roll if you must, otherwise pick
a legal move at random.** Random is a bad outcome for the player it happens to
and a much better one than three other people waiting on an empty chair.

A test walks sixty games and asserts that any position offering the human no
way to pass is one of the four phases named above, so a new blocking phase
fails the suite rather than quietly stalling a game.

**The forfeit draws from its own generator**, seeded from the game's seed but
separate from it, so a forced move never disturbs the dice or the deck, and the
same seed with the same timings forfeits the same way.

**An unanswered offer is a refusal.** An offer left on the table stops the bots,
because they are waiting on an answer. That wait belongs to whoever owes the
answer, not to whoever holds the turn, so the clock charges it to them and
declines for them when it runs out. Getting this wrong stopped the game dead:
the clock ran against the seat whose turn it was, and enforcement only ever
looked at the decider, so nothing was ever forced and nothing ever moved.

### One clock, above the players

A single large figure with whose turn it is and the turn number beneath it. It
follows whoever is on the clock, not whoever is reading: you need to see the
other side thinking as much as you need to see yourself run out.

It used to be a figure on every seat row, which made four small numbers where
only one of them was ever moving.

**The turn counter is the session's**, not the engine's. The engine tracks
whose move it is, not how many moves have gone by. Setup is dealt rather than
played and is not counted.

Enforcement is lazy. A server that only wakes when asked cannot act on the
second, so it happens on the next request, and the page's existing poll is
what makes that arrive.

**A per-turn allowance must not refill mid-turn.** Time is settled against a
seat whenever anything happens, but the turn's own clock restarts only when the
turn changes hands. Getting this wrong made a clock roll for the player and
then hand them a fresh minute for it.

---

## 9. Components

The interface is built on **shadcn/ui's token vocabulary and component
anatomy**, ported as plain CSS. shadcn is not a package. It is source you copy
and own, so what is borrowed is the naming and the structure, not React,
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
explanation becomes unreachable by keyboard and by touch, precisely the
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
from the upper left, one hue each. The same idiom as the road, settlement and
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
