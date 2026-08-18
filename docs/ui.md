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
│  header, the mark · this game's name · its report            │
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

**Centre: the board.** It is also an input device, see §5.

**The robber lifts to be looked under.** It stands over the number disc almost
exactly, so the one thing worth knowing about the hex it has shut down is the
one thing it hides. Hovering it raises it clear of the disc: 31 board units,
which is a lift rather than a nudge, because a nudge would reveal nothing. It is
slower than the tiles' own hover, since this is a deliberate look and not a
pointing cue.

**A road covers 70% of its edge**, which leaves a gap at each end so two roads
meeting at an intersection do not run into each other. The vertical drawing
takes a tenth less. It is the only one whose bar runs straight down the screen
with nothing foreshortening it, so at the same share of its edge it read as the
long one of the three. Its balance point is scaled with the shortening, so the
bar stays on the same line rather than rising off it.

**The board never moves.** Its frame is measured from the land and the ports
alone, which do not change for the life of a game. Measuring everything drawn
meant that a piece placed past the previous extent shifted and rescaled the
whole board, which is the one thing a board must never do. The only thing that
resizes it is the window.

**The header is the mark, the table's name, and the way through to the
report.** Whose turn it is, the turn number and the seed all belong to panels
that already say them, and repeating them at the top made the one strip that
never changes the busiest thing on screen.

The mark links home; the name sits beside it in the body face and a quieter
colour, so the mark stays the mark and the name reads as the answer to "which
game is this". An unnamed table shows nothing rather than a placeholder, since
a name nobody chose is worse than no name.

**The report is the one other thing up there**, and it is the only exception to
the rule above. A game has exactly two pages, the board and its analytics
(§12), and the header is where a page says what else there is. It sits at the
far end, so the mark and the name stay together as one thing and it reads as
somewhere else to go rather than as more of the title. It works mid-game: a
report on a game still being played is a report on the position so far.

**It has no surface of its own**, and no rule under it. It had a paler
translucent panel with a blur behind it, which is what you build when content
scrolls underneath; nothing scrolls here, so it was separating the title from
the table for no reason and reading as chrome. Bare, it takes the page's own
colour, the page's own grain and the pools of light that wash across the top of
it, and the mark simply sits on the table like everything else that is not a
card.

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

Five stacks, one per resource, always in the same order, **drawn as fans of
cards, one card per card held**, and carrying a count badge on the pile's top
right corner, where every other count on the page sits. Empty types stay in
place, greyed at zero.

The depth is the point: it tells you a fat hand is coming before the
discard-on-seven rule bites, and it does it without you reading a number.
Fixed slots mean the thing you want never moves under the cursor.

The fan stops growing at five cards and shares a fixed budget of width between
however many it draws, the way a real fan of cards does. Past that the badge is
doing the counting anyway, and a hand of twelve spread at full width would push
the dock wider than the board that sets its width.

Development cards sit in their own group beside the resources. **You play one
by clicking it**, so there is no separate button opening a list of the cards
you are already looking at. A card bought this turn is marked `new` and cannot
be played until the next one (R-9.4): the engine refuses it either way, but a
refusal you cannot see in your own hand reads as the interface being broken.

**Before the roll, only the militia is playable** (R-9.5), and the other piles
say so rather than simply declining to click. It is the one card whose timing
changes anything, since moving the robber before production decides which
hexes pay this turn.

**Every pile's tooltip says what the card does**, in one sentence taken from
the rule it implements (R-9.7 to R-9.11). The face carries a name and a number,
which says what you are holding and not what happens when you play it, and
there is nowhere else on the screen the effect is written down. The awkward
corners are named rather than smoothed over, since they are exactly what a
tooltip is for: two roads come out as one on a board with room for one
(R-9.10a), and a monopoly may name a resource nobody holds and yield nothing
(R-9.9).

The sentence is followed by why the pile will or will not click, and there are
three reasons it will not, told apart so the tooltip never explains a refusal
that is not the one in force: bought this turn (R-9.4), before the roll and not
a militia (R-9.5), and a card already played this turn (R-9.3). The third had
nothing on the card to show for it and used to say nothing at all.

**A victory point card is the exception, and is not a card you play.** It
scores the moment it is bought, it is yours alone until the game ends, and no
action exists behind it (R-9.11). It carried the `new` badge and offered
itself as playable next turn, which described a card that does not exist. Its
pile is inert now and says what it is instead.

**A card in your hand shows its own face**, from `art/dev-*.svg`: the card's
name, its drawing, and what it is worth. It stood for a while as a ring on a
blank card, which is a card back, and a card back in your own hand says the one
thing that is never true of it, that you have not read it. Everywhere the card
is genuinely unknown it stays a back: the deck in the bank, and the line in the
log that says somebody bought one.

**Both groups are the same object.** A development pile is drawn by the same
code as a resource pile: one fan, one card per card held up to the limit, the
same edge, the same tooth, the same badge, the same empty slot. The only thing
that differs is what a card is painted with, the terrain's colour and a disc, or
the card's own orange and its face. They were two ways of drawing a hand of
cards for a while, an up-and-right pile beside a fan, which meant a hand that
changed shape halfway along the row.

Two states then had to be said differently, and both for the same reason: the
card's ground is the accent, so the accent is no longer available to mark it
with. `new` dashes the card's own edge, the way an empty slot's edge is dashed.
Playable rings the whole pile rather than each card in it, since a ring per card
drew one around the front card and another around every sliver behind it, which
read as three piles rather than one that is live.

### The two awards

Longest road and largest militia (R-10) are **cards you hold**, sitting to the
right of the development cards. That is what they are from where you sit:
something you have, that is worth points, that somebody can take off you. They
are drawn by the same face template the development cards use, out of the same
two drawings, arranged to say "longest" and "most" rather than "a road" and "a
robber": three roads end to end, and three militia with the middle one forward.

That arrangement is the whole reason the drawings are rearranged rather than
reused as they are. Road building shows two roads set *across* each other so
they read as two roads; longest road shows the same drawing end to end so it
reads as one road that keeps going. One robber is the militia card, and the word
underneath would have been the only difference.

**Only your own, and only when you hold one.** Who else holds what is on their
seat row, with the rest of what is public about them. A slot held open for a
tile most games never give you is a lot of dock for nothing, so the section is
not there until it has something in it, and the layout moves when it appears.
That is the one place the dock breaks its own fixed-slots rule, and it is worth
it.

**No count badge.** There is one of either, and a number that can only ever read
`1` has nothing to say.

The tooltip carries what it is worth, where you stand right now, and what it
takes to lose it: five roads to win the road and strictly more than the holder
to take it, three militia played for the other (R-10.1, R-10.6, R-10.8).

### Actions

A fixed grid: **roll, buy a development card, trade, end turn.** Four of them,
so the grid is **two by two**. In a row of three, the fourth hung alone
underneath and read as a leftover rather than as one of a set.

**Building is not here.** It has a section of its own, next door. The grid is
for actions with no place on the board; building has one.

A button that is unavailable **stays put and greys**, the layout is learnable
only if it never changes. **The reason appears on hover**, so a quiet dock
does not cost you the explanation.

### Building

Three buttons, one per piece, in the order they cost: road, settlement, city.
Each carries the board's own drawing of that piece, so the button and the thing
it puts down are the same object seen twice.

**A button you can press shows the piece in your own colour**, because that is
the piece it puts on the board: press it and the ghosts that appear are that
colour too. A button you cannot press shows the same piece in a warm grey, which
is the palette's way of saying a thing is not in play. Either way the colour
goes through the drawing's own lighting ramp rather than being picked face by
face, so the piece on the button is lit exactly like the piece on the board.

**Pressing one arms it; the board answers with the spots for that piece.** Every
spot you could afford used to be lit the whole time, which turned an ordinary
turn into a board covered in pieces nobody had asked about. Arming commits to
nothing and spends nothing, so it costs nothing to press.

**It stays armed while that piece is still something you could build.** Two
roads is two clicks on the board rather than four clicks in all. It disarms by
pressing the button again, by clicking anything that is not a spot, or on its
own the moment the engine stops offering that piece: the last one you could
afford has gone down, the spots have run out, or the turn has ended. That last
one is read off the choices rather than tracked, so there is no second idea of
what is buildable to fall out of step with the engine's.

**A button is live when there is a spot, not when you can pay.** Holding a
road's worth of cards on a board with nowhere legal to put one would arm to an
empty board, which reads as the click having missed. Why a dead one is dead goes
in the tooltip, in the order you would go and do something about it: whose turn
it is, then the dice, then the pieces left in your supply, then the cost, then
the board.

**Two placements are not asked for, because they are owed.** Setup, and the
roads a played road building card still has to put down (R-9.10): the rules give
no way to leave either position, so their spots show without a button. That is
the same argument the decision cards make about a forced question.

**Two by two, and no words on them.** The same block the actions beside it
make, out of the same button: measured, a build button and an action button are
the same box to the pixel, which is what makes the two sections read as a pair
rather than as two different ideas that happen to be adjacent. Three pieces in
four places leaves one empty, and that is the price of the pairing.

The drawing takes the whole button, at the height an icon and a line of label
come to next door. The name is in the tooltip and on the button as its
accessible name, so it is there for a screen reader and for anyone who does not
recognise the piece.

Named buttons were tried first, three across and then stacked in a column. Three
across are wider than the dock has to give at a laptop's width, and a fifth
section that wraps the whole dock onto a second row every turn is a bad trade
for three words.

The dock's type scale came down with it, from `.8vw` to `.74vw`. The dock sizes
itself off the window and so does the space it has to fit into, so between the
clamp's two ends the two grow together and whether the row fits is very nearly a
constant. A fifth section pushed that constant just over one, which wrapped the
dock on every laptop rather than on small windows only.

### Dice

Two **square** dice showing the last roll, **one above the other** in the dock's
right corner, standing **the full height of the cards beside them**. Two dice
and the gap between them come to a card's height, and both numbers are written
down once rather than one being derived from the other at layout time.

Stated rather than computed on purpose. Sharing the column's height between the
two with `flex` and taking the width back from `aspect-ratio` also produces a
square, and does in Chromium, but it asks a replaced element for a width based
on a height it was itself handed by flex growth. That resolved to the width of
the word `DICE` in another engine and drew a pair of tall thin cards, and even
in Chromium it left the dice overflowing a column measured at less than half
their width, which quietly took fifteen pixels off the dock's own accounting. Clicking
rolls. Side by side they were the widest thing in that corner and pushed the
actions off the middle of the dock; stacked and stretched, the pair is one die
wide and as deep as the dock is, which is the whole point of a die being an
object rather than a number.

**One gap between every group**, the dock's own. The dice were held off the
controls by more than that for a while, on the reasoning that a die is not
something to press; side by side with building and actions a group apart, the
odd one out read as a mistake rather than as an argument.

### The labels sit on one line

Every group in the dock hangs from the top, so `resources`, `development`,
`actions` and `dice` all start at the same height. They used to be aligned to
the floor of the dock, which is right for the cards and wrong for the headings:
a group with less in it had its label pushed down to wherever its own contents
happened to end, so four headings sat at three different heights and the dock
read as three strips rather than one. Inside a group the cards still stand on
the floor.

---

## 4. Seat rows

Every row carries, in this order: colour, name, public victory points, cards
in hand, development cards held, a badge when that seat holds the longest road
or the largest militia, settlements / cities / roads remaining, and militia
played / longest road.

Pieces remaining is on the row because it is a real signal. A player down to
their last settlements is close to winning, and it is the one the raw numbers
hide.

**The bonus counts are shown for everyone, not only for whoever holds them.**
The badge says who has the award; the numbers say how close the rest of the
table is to taking it, which is the thing you actually play against. A seat one
road short of the longest is a different position from a seat four short, and
the badge alone cannot tell them apart.

## 4a. The bank

Under the clock, one row per resource: the card, its name, and what is left of
that stack. A resource the bank has run out of is marked, because that is the
one worth noticing: it stops production for everybody until something is spent.

**The development deck is the sixth stack**, set a little apart by a rule: it
belongs to the bank but is not one of the five and is counted out of a
different number, twenty-five rather than nineteen. It is finite and never
refilled (R-9.6), so running down is the whole story of it, and a table that
cannot see that coming is guessing. It wears the card back and its mark rather
than a terrain colour, because a development card is not a resource.

**Two modes, set in the lobby.** `exact count` shows the number, and is the
default because supply counts are public and any player could work them out by
watching (R-5.6). `stack size` shows only how full the stack looks, big,
middle, small or empty, for a table that would rather judge than tally. The
bands are thirds of a full stack rather than round numbers, so they mean the
same thing whatever the stack started at.

### The log can be turned off

A table rule, not a personal setting: playing from memory only works if nobody
has the record. With it off **the server does not send one at all**, because
hiding it in the page would leave the history sitting in the response for
anyone who opened the network tab, which is not playing from memory.

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

**Public resource movements are logged.** Production on a roll, the grant from
the second settlement, bank and port trades, and trades between players, each
saying what actually changed hands. "Took an offer" told you nothing; it now
reads "Take 3 wool from Odd for 3 wheat".

**A robber steal is not.** The card moves and the table sees that it moved, but
which card it was is not public. Reporting it through the same hand-diff that
reports production would leak it, so the actions that can pay out in the open
are named explicitly rather than inferred from a hand changing.

**The roll leads with the total**: `Rolled 9 (4, 5)`. The total is the number
the board answers to; the two dice follow. `Roll 4 and 5, 9` read as three
numbers of equal standing, which is not what a roll is.

**Resources are drawn, not named.** A line carries a fan of cards where it used
to carry a noun and a number, counted by looking exactly as the dock asks, so a
reader scanning a turn sees colours rather than a wall of words. Past what can
be told apart at that size the number comes back, because a fan nobody can
count is worse than the digit it replaced. The roll shows its two dice instead
of writing them in brackets. **A development card is drawn as the bank draws
it**, the orange back with its white disc: buying one and the deck it came off
should be recognisably the same card, and "a development card" was the last
card-shaped thing in the log still being spelled out.

**A handful of discards is one line.** The engine discards one card at a time,
so handing four back was four lines saying almost the same thing; what happened
was a single decision about a hand, and it reads as one.

**No board indices.** A button has to tell two otherwise identical choices
apart and so carries the vertex or edge; the log does not. The board already
shows where the road went, and "Build road at 68" asks the reader to hold a
number that means nothing to them.

**Scrolling up stays up.** The log followed its own tail on every poll, so
reading anything older than three seconds was impossible. It only follows the
tail if you were already at it, and it keeps its distance from the bottom
across a poll: emptying the box to redraw it collapses the height and takes
`scrollTop` to zero, which threw a reader back to the opening deal every three
seconds.

**The whole record is sent, not a tail of it.** The payload used to carry the
last forty lines, which meant scrolling back far enough simply ran out of
game. A log you cannot follow to turn one is a log that misrepresents the
match, and the page has no other source for what happened there.

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

**Whoever is in front has their score in the accent**, and ties mark everybody
level at the top. Colour on the number rather than a tag beside it: the standing
is what the number already says, so it is the number that says it louder.

It is judged on **the numbers actually printed in these rows**, which is not one
measurement: your own is your real total and every other is what the table can
see. It was decided on the public count for a while, which is internally tidier
and read as broken, because two rows both showing 5 had one of them accented and
the other not. A reader compares what is on the screen, so the screen has to be
what is compared. The mark means "in front as far as you can tell", which is the
only standing a player actually has.

**Nobody is marked while everybody is level**, which is the whole table for the
first few turns. Four accented numbers would announce four leaders about a game
where the word does not apply yet.

---

## 5. Where a decision happens

The rule: **anything with a place on the board happens on the board; anything
without one happens in a card that waits for you.**

| Moment | Where |
|---|---|
| Build a road, settlement, city | Dock, press the piece; then board, click the ghost |
| Setup placement | Board, click the ghost |
| The roads a road building card owes | Board, click the ghost |
| Move the robber | Board, hexes light, click one |
| Choose whom to rob | Card, by name. The robber is already on the hex |
| Discard down to seven | Card, the whole hand at once |
| Monopoly / invention resource pick | Card |
| Bank and port trades | Card, inside the trade composer |
| Incoming trade offer | Card |

The card is a **real modal, centred over the board.** An earlier version put it
in the column held for chat, which read as a side panel rather than as a
question: it sat where a running conversation would sit, at the edge of where
anyone was looking, and it competed with the log for the same glance.

**Nothing moves because a question arrived.** The card is `position: fixed`
rather than part of the layout, so opening one leaves the seats, the board, the
reserved column and the dock exactly where they were. That was already the rule
when the card lived in a column, and it survives the move: a panel that
resizes what the player is reading is worse than one that covers a corner.

### More than one card can stand

**One card per thing being asked of you.** They stack in the board's corner,
under each other, and close the gap when one goes. The stack is a flex column,
so that last part is free; `#prompt` is the stack and carries no surface of its
own, and each card is a `.promptCard` with its own `role="dialog"`, its own
title and its own close button.

It used to be one card that could say exactly one thing, chosen by priority.
The commonest pair was also the worst served: opening the trade composer hid the
incoming offer, so the offer you were answering vanished the moment you went to
counter it.

**The stack does not scroll and does not clip.** It used to be the scroller,
with padding around it so the cards' shadow had somewhere to go, and that cannot
work: the shadow is a 60px blur, no amount of padding is enough, and it was cut
off square. What should have been a soft glow read as a hard grey slab sitting
beside the card. A card caps its own height and scrolls its own contents
instead, which clips descendants and never the element's own shadow.

The order, top to bottom, is **what the rules are forcing, then what you opened
yourself, then what the table is asking.** Yours sits above the market so an
offer arriving cannot push the thing you are working on down the screen. Two
cards are exclusive and stack with nothing: the end of the game, and a seven.
There is one thing to do in each, and standing anything beside it would offer a
choice that is not there.

**A card only stands when the thing it asks for is yours to give.** Keyed on
whether the human is the decider, never on the phase alone: a bot playing a
militia puts the game in `MoveRobber` too, and reading the phase told the player
to place a robber that was not theirs to move.

### Taking back a half-played card

A development card that opens a second decision is spent on the first of them,
and between the two the player has committed to nothing and learned nothing.
That gap can be undone.

- **A card that only asks a question** (monopoly, invention) has spent nothing
  when its picker opens, so closing it changes nothing.
- **A militia** is already played by the time the board is asking where the
  robber goes. Clicking anywhere that is not a hex puts the card back, which is
  the only way out of a placement with no button of its own. Escape does the
  same.
- **A card that arrived because something was already played** says `put the
  card back` rather than `close`, because that is what pressing it does.

The engine keeps the whole position from before the play, its generator
included, so what comes back is the same position down to the next random
number: this cannot be used to fish for a different steal by playing the card
again. The snapshot is guarded on the live position rather than cleared at
every place a move can come from, so no path nobody thought of can leave a
stale offer to undo something else.

**A card belongs to the turn it was opened on.** If the clock runs the turn out
from under you, the card goes with it: leaving a composer or a picker on screen
invites you to answer something nobody is asking any more, and the click would
be refused as stale anyway. The turn number is the signal, because that is what
the server advances when it forces an end.

Covering part of the board is the accepted cost. Every choice on that list is
one you make against the position, so the card is the width of a question and
not of a page, and the board stays live behind it: it is `role="dialog"` but
deliberately not `aria-modal`, see §9.

---

## 6. Trading

**Starting:** click a card in your hand. The composer opens with that card
already on the table, so wanting to trade a wheat and saying so are one
gesture rather than two. `Trade` in the dock opens it empty, for when you know
what you want before you know what you are giving.

**The composer is docked to the dock**, not stacked in the board's corner with
the other decision cards. A trade is made out of the cards in your hand and
those are at the bottom of the screen, so it is built next to them: the bar is
the dock's own width and depth and sits directly on its head. It is measured
off the dock rather than given a size of its own, so there is nothing to keep in
step through a window resize.

The hand stays visible and clickable underneath, which it has to be, since
clicking it is how the offer gets built. The bar overlays the bottom of the
board, which is the part of the board that is sea, and nothing in the layout
moves when it opens.

**Composing:** click a stack in your hand to move one card into *you offer*;
click that card in the tray to take it back. Asking is the same gesture against
a row of five, always all five, since you may want what you do not hold. Both
sides are **cards, not steppers**: a stepper row names a resource in words and
asks for a number, where the cards are the game's own word for a resource and
are already on the screen twice over, so putting one up is picking it up and
putting it down.

**Every card that moves in or out of the composition makes the card sound.**
These are the one set of gestures the log never sees, since nothing has happened
at the table yet and there is no line for a cue to hang on, so they are the one
exception to sounds being played off the log (§7a). A card you pick up should
sound like the cards that get dealt to you.

**The recipient sits against the words it belongs to**, beside *you offer*
rather than pushed to the far end of the caption, where it read as an unrelated
control that happened to share a line. The select is drawn by the page and not
by the platform: the colours and the border were being set while the browser
still painted its own control over the top, a grey ramp and a pair of stacked
chevrons, which made the one select on the screen the one thing that looked like
it came from another program.

**The two trays are one pair and sit at the same height.** The ask side carries
a row of five beneath it and the offer side does not, so the offer side is given
that row back as an invisible copy of it. A calculated height was two pixels
out, because a row of cards is a row of cards plus whatever the buttons around
them add, and a copy of the thing is exactly as tall as the thing. The captions
are a fixed height for the same reason: one carries a select and the other
carries nothing.

**The cards are centred in the tray, and the row of five is centred under it.**
A tray is half the dock wide and usually holds a card or two, so packed into its
left corner they read as the start of a list that has been cut off rather than
as what is on the table. The tray keeps the slack, not the cards: it still
stretches to fill its side, and what is centred inside it is the cards.

Their size is set by what the bar can give **two** rows of them. A caption, a
tray and the row of five all have to fit inside the dock's own depth, which is
what the bar is measured from, and at the tightest step of the type scale that
leaves about 2.7em each. A sweep of window sizes asserts the cards stay inside
the tray's outline rather than the tray being squeezed out from under them.

**Putting an offer up leaves the composer open.** An offer to the table is the
start of something rather than the end of it: the answers come back one at a
time, and a counter or a second offer is the commonest next thing to do, so the
tool for making one should still be in your hands. The trays empty themselves,
because proposing moves the version on and a composition is about the position
it was built in.

**A bank or port trade does close it.** That one is finished the moment it is
pressed: the supply always takes it, there is nobody to answer, and there is
nothing left to say.

**Clicking away puts it away.** Nothing has been committed while it is open, so
turning your attention to the board is an answer. The dock is not "away":
clicking your hand is how the offer gets built, and the composer sits directly
on the dock precisely so the two read as one thing. Nor is a decision card,
which can stand at the same time and is a separate question.

That listener runs on the **capture** phase, which is load-bearing. Clicking a
card in your hand rebuilds the hand, so by the time a bubbling listener ran, the
clicked node had already been thrown away and `closest` walked up from a
detached element to nothing: every click on the composer read as a click outside
it and closed it.

**One door for every trade, and three ways out of it.** The bank had its own
button in the dock opening its own list of twenty sentences, which split trading
in two and made the trade people make most often the one they could not simply
say. **Trade** opens the composer whether the other side is a person, a port or
the bank, and the same composition answers all three buttons:

| Button | Lights when |
|---|---|
| Offer trade | Both sides have something in them |
| Bank trade | What you built is one resource at the bank's rate for one of another |
| Port trade | The same, at a rate a port of yours beats the bank with |

A bank or port trade is one resource in and one out, which is exactly the shape
the composer already builds, so you choose the route rather than the form. **All
three stay live at once**: a player may beat the bank, and four wheat on the
table is a fair thing to ask the table about even when the bank would take it.
A button that cannot be pressed says what it would take, so the option is never
invisible until you stumble onto the right count.

Which rate belongs to which button is read off what the engine is offering
rather than written here: four is the bank's, and anything under it is a port's.

**Receiving:** an offer arrives as a card that waits for an answer, accept,
decline, or counter. It does not sit quietly in a panel to be missed.

An offer reads **"Ines offers 1 wood for 2 wheat"**. It used to be "Ines: 1
wood for 2 wheat", which left the reader to work out which side was which.

Each offer carries **the waiting loader in the proposer's colour**, beside their
name. It is the same animation an empty seat uses, and for the same reason:
somebody is waiting on an answer. The motion walks the eye from who asked to
the buttons that answer them.

### Handing half of it back

A seven is a decision about the **whole hand**, so the whole hand is on screen:
every resource with what you hold and what you are choosing, a running count
against what is owed, and a confirm that stays shut until the two agree.

One button per resource discarded a card the instant it was pressed, which
asked the player to make the decision one irreversible card at a time without
ever seeing what it added up to. You also cannot choose more than is owed, which
is what makes the confirm a yes or no rather than something to be talked down
from.

The engine discards one card at a time, so a chosen hand is played out as a run
of single discards, **sequentially**: each one moves the version on, and firing
them together would have every card after the first refused as stale.

**Silence is a refusal.** An offer left on the table stops the bots, because
they are waiting on you. Nothing about that wait is charged to you: it runs
against the clock of whoever holds the turn, and when their turn runs out the
offers die with it (§8).

**Every offer put to you shows, including the ones you cannot cover.** Two
different things used to look identical from the outside, and only one of them
was correct:

- An offer made by a passive player on somebody else's turn is addressed to the
  active player alone (R-7.3). It is not yours to answer and there is nothing to
  show. This is right and stays.
- An offer that *was* put to you but that you cannot cover is a question. It
  used to be silently invisible, because the whole card was keyed on what you
  could accept: no accept, so no card, no explanation, and no way to say no. The
  table then waited on an answer you had no means of giving.

So the card is keyed on **whether you were asked**, which is a rule about seats
and not about hands, and the accept button on whether you can cover it. One you
cannot cover shows, has no accept button, and can still be turned down.

Nothing labels it as unaffordable. There was a tag saying so, from when such an
offer was invisible and the row had to explain itself; now that the offer is
drawn in cards and your hand is drawn in the same cards a few inches below, the
missing button is the whole of the message.

The two are still distinguishable at a glance, because an offer nobody put to
you never appears at all.

**A beat is spent on a move, never on looking for one.** Bots share one pace
gate for everything they do, moving and answering the market alike, so the two
read as one table thinking at one speed. That means settling the market has to
work out who would take what *before* it arms the wait. Arming it first spent
the beat whether or not there was a trade to make, and an offer nobody wants
sits on the table for the rest of the turn: every tick went on a trade that was
never going to happen, the seat whose turn it was never got a beat to move in,
and the table stopped dead with its clock at zero.

### An offer is shown in cards

**The trade is drawn, not written.** "1 wood for 2 wheat" has to be read and
then matched against a hand that is drawn as cards a few inches below it. The
cards are the game's own word for a resource and they are already on the screen
twice over, in the dock and on the board, so an offer said in them is taken in
at a glance rather than parsed.

**Every way out of the card sits in one row along its foot.** Accept, counter
and no thanks used to be in three places: two inline against the offer they
belonged to, one alone under a rule. That let the row's width decide whether the
buttons wrapped under the sentence, and it read as three unrelated controls
rather than as the set of answers to one question.

### The round of replies

An offer is answered by **one seat per beat**, and each answer is shown as it
lands. Everyone the offer was put to starts at `???` and turns into *accepted*
or *declined*.

**Answering is slower than moving**, and drawn from its own window: about a
second to two at *fast*, two to four at *slow*, against a move's half-second.
A move is watched; an offer is read, weighed against a hand, and possibly
contested, and all three have to fit before the table has settled it. At a
move's beat an offer was gone before it could be reached for, which made the
market something that happened to the player rather than something they were in.

One at a time rather than the whole table at once, because three refusals
landing together is a verdict and three arriving in turn is the table thinking.

**A proposal of yours stands until it has been answered, and then closes.**
Before this it went on the table and said nothing back: you got a result or you
got silence, with no way to tell which was still coming. So it stays for the
round of replies, and goes when the round is over, which is either of:

- **Somebody took it.** The cards have moved, the hand shows it and the log
  names who took it. The seats that had not answered never will now, so leaving
  it up meant a row of `???` against a question nobody is being asked.
- **Everybody turned it down.** The replies are all in and the log has every
  one of them.

Either way the question is closed, and a card that stays up after it has been
answered is a card asking nothing. If neither happens the turn takes it, along
with the offers themselves.

**Everyone asked answers, including seats that could not have covered it.** A
seat that cannot afford an offer says no like anybody else, and the answer never
says why. Reporting "cannot" instead would publish something about a hand that
§7.3 keeps private, and a seat left at `???` for the rest of the turn would
publish the same thing by omission. Seats the offer was *not* put to (R-7.3) are
not on the row at all, which reads off seat numbers and needs no hand.

**The record outlives the offer.** The engine drops an offer the moment it is
taken, which is exactly when there is most to say about it, and reindexes what
is left by swapping the last entry into the gap, so an index is not a name for
an offer. The session therefore keeps its own list of the turn's deals and
matches it back to the engine's table by value.

---

## 7. The roll

A roll is shown, not reported. The dice settle on the number, the hexes that
match light up, and **resource cards travel from those hexes to the hands that
earned them**, into your stacks, into the opponents' rows. You see who got
paid and from where without reading anything.

Bot turns play out at whatever pace the lobby was set to (§8).

---

## 7a. Sound

**Eight sounds, and each one names an event rather than an interaction.**

| Sound | Plays when |
|---|---|
| `confirmation-001` | The table is dealt · your turn begins · a trade is taken |
| `dice-throw-3` | The dice are rolled, by anyone |
| `impact-generic-light-002` | A piece goes down on the board |
| `card-place-1` | Cards are dealt into a hand, or moved in the composer |
| `drop-002` | An offer goes on the table |
| `error-008` | An offer is turned down |
| `jingles-hit-10` | The game ends and you won |
| `jingles-hit-15` | The game ends and somebody else did |

`confirmation-001` carries three of the eight, which is worth watching: a cue
that means three things is a cue that means none of them. What holds it together
is that all three are the same message, *you are up*: the table is ready, the
turn is yours, the trade went through. The three contexts are far enough apart
that none of them is ambiguous in the moment. If it ever does get
confusing, the turn cue is the one to move.

**The deal is the one cue that is not about you at all.** Every page plays it
when its game begins, so when the other seats have people in them the whole
table hears the same thing at once and knows to look up. A dealt game is a
different seed, or a version counter that has gone back to nothing: the seed is
the real signal since dealing is what mints one, and the version is the belt to
its braces for a table re-dealt from a seed it was already playing.

**They are played off the log, not off the clicks.** The log is the record of
everything that happened, including everything three bots did while nobody was
looking, so hanging the cues on it makes a move sound the same whoever made it.
Hanging them on the click would have made the whole table silent except for you,
which is the opposite of the point: the sounds are here so a player can tell
what is going on without watching every corner of the board.

**Two are about you rather than about the table**, and those two are the ones
that do not come off the log: your turn beginning, and the game ending. The log
says what happened; these say whose move it is and how it went for you.

Your turn is read from the **turn counter and the active seat**, not from
"you are the one deciding". That second thing is also true when a seven asks you
for cards in the middle of somebody else's turn, and a fanfare for being robbed
is not the message. A turn of yours is a new turn number with you holding it,
which is what the counter and the log already group by, and it counts each
placement in the deal as the turn it is.

The ending is kept apart from the turn cue rather than folded into it, because a
game can end on somebody else's turn and there is then no turn of yours to hang
it on. Which jingle plays depends on which side of it you are on: this browser
is one player, and the end of a game is not the same event for everybody.

**One flag decides whether a payload is the first one read**, shared by all
three readers. Each used to use its own state as the marker, and since they run
in a fixed order the turn reader seeded itself first, which told the ending
reader that this was not the first payload after all: a reload onto a finished
game played the jingle for it.

So a "dealt card" is any line that puts cards into a hand: production, a card
bought, a card stolen. A "placed piece" is any line that puts wood on the board:
a settlement, a city, a road, and the robber, which is the militia's whole
effect and a seven's. Within each group they are one event to the ear, and one
sound saying "a piece went down" is worth more than four saying which piece.

**The market is the exception, and gets three cues rather than one**, because a
trade is a small conversation and its three moments are the ones a player most
needs to hear from across the board: the offer going up, the yes, and the no.

**The first pattern that matches a line wins**, so the specific ones come first.
Two pairs turn on that: a trade taken is not the same event as a card being
dealt even though a trade deals cards, and a card stolen by the robber begins
with "Took" exactly like a trade does.

**A move the clock made for you is the same move.** The log writes it as "Time
ran out, placed a settlement for you"; the prefix is stripped before the line is
matched, so a forfeited placement sounds like a placement.

**At most one of each kind per payload.** A roll that pays four players writes
four lines and is still one thing to listen to; four card sounds landing on each
other is a noise rather than a cue. They fire in table order when several land
together: the dice, then whatever went down on the board, then the cards that
moved because of it, then anything put up for trade afterwards.

**The first read is silent.** Reloading mid-game arrives with the whole history
in hand, and playing it back would be a minute of dice.

**On by default, off in one press, and the press sticks.** A cue nobody switches
on is a cue nobody gets, so the default is on; a game that makes a noise you did
not ask for should be one press from stopping, so the control is on the screen
and not in a settings page. Turning it back on plays one, because that press is
also the only way to find out whether this machine makes a sound at all.

The toggle lives in the **board's top right corner**, mirroring the decision
cards in its top left. It is a setting of this player's own and not of the game,
so it belongs on their board rather than in the header, which names the table
everyone at it shares. It sits above the cards in the stacking order: a card
standing across the whole width of a narrow board must not be the thing that
stops you turning the sound off.

A browser refuses to play anything before the page has been interacted with, and
refuses by rejecting a promise rather than by throwing. The lobby's own buttons
are that interaction in practice. A refusal is caught and dropped either way: the
board should never notice that the audio did not work.

The files are Kenney's, CC0, carried in the binary like the fonts and served
from `/sound/`. `audio/SOURCES.md` records which pack each came from and how it
got here.

---

## 8. The lobby

A **setup screen before the game**, not a row of dropdowns above it. The board
does not exist yet when you are on it.

**Two columns, split by what the setting is about.** The left is the table:
what it is called, who is at it, how fast the bots move, who can find it, the
link, and which board it deals. The right is the game: the size of the table,
chat, the turn clock, the bank and the log. One long column asked a reader to
hold that distinction in their head instead of seeing it. Below 720px it
becomes one column in the same order, because a narrow window should get the
same screen and not a different one.

It carries: your name, the size of the table, who holds each seat, whether the
table is listed, a share link, the turn clock, whether the table keeps a log,
and a seed. It stays configurable **while people join**, which is what makes it
a lobby rather than a settings dialog.

**A table is private unless it asks to be listed.** Listing is publishing, and
publishing is the answer that cannot be taken back, so it is the one that has
to be chosen rather than the one that happens by default. The server agrees
independently: anything other than an explicit `public` leaves the table
unlisted, so a missing or misspelled setting cannot publish a game by accident.

Public was **shown but not selectable** for as long as there was no page for a
listed table to appear on. There is one now (§13), so the setting does what it
says: a listed table is on the home page for anybody who can reach the server,
and an unlisted one is on its host's copy of that page and nowhere else.

Visibility is stored on the session rather than in the browser that dealt it,
because a listing is read from the table and not from whoever is looking at it.
The home page reads it off the session, which is why that was the right place to
put it before anything read it at all.

**The lobby has an address of its own: `/lobby`.** It is this page served with
no game behind it, which is what every **New game** link leads to. There is no
board to draw, nothing to poll and no report to link to, so none of that runs;
what is left is the lobby, which is a screen of this application rather than a
second form somewhere else to drift out of step with this one.

It is **a page rather than a sheet over one**. It used to be `position: fixed`
across the whole window, which covered the header and made it carry a wordmark of
its own inside the card. It sits in the flow under the application's header now
(§14), the board's screen is hidden rather than lying behind it, and the card's
title says what the screen is, **New game**, rather than repeating the mark above
it. It paints no ground of its own either: a page does not need to paint over
itself, and the wash and grain it used to lay down are the page's already.

**The lobby is never opened over a board.** It used to appear over any board with
no moves in it, because arriving at one meant the server had dealt it and nobody
had said what they wanted. A table is asked for now, so opening the lobby on top
of it would ask the same questions twice, and it has an address, so every way to
reach it is a link to that address: the header, the end of a game, and the home
page all point at `/lobby`. Its way out, **Back to the home page**, is a way out
rather than the sheet closing, which the auto-opening version never needed because
the only exit was to deal.

**Seats hold bots by default and can be left open.** Waiting for people was the
default while the lobby was a side door reached from a board somebody was already
sitting at; it is the front door now, and a front door whose one button refuses
until you have pressed three others is the wrong first screen. So the default is
bots, and opening a chair is a click a seat.

**Dealing with an open seat used to be refused** and is not any more, because the
refusal was about the server rather than about the table: it could not seat a
person, so dealing would have quietly turned a waiting seat into a bot. It can
(§16), so the button says what it will do, **Deal, and hold a seat**, and does
it.

**Your name is editable because nobody is signed in.** It is kept in the
browser between games, which is the closest thing to being remembered without
an account. When there are accounts the name comes from one and the field goes
away; the server already stores it per session rather than the page holding it,
so that swap does not move anything.

**The board cannot be dealt with an empty seat.** Dealing would quietly turn a
waiting seat into a bot, which is a decision about who is playing being made by
a mis-click. The button says how many seats are short and refuses until they
are filled or the table drops to three.

**The share link is the table, as a link.** It was labelled *Invite* and pointed
at `/join?table=<seed>&seats=N`, which the server had never implemented: the one
control on the screen promising to bring somebody else in returned a 404, under a
tooltip admitting the joining was not live. The link was not real either.

It is real now, and it is the same description of a table that dealing one uses.
The page builds that description once; pressing **Deal the board** posts it to
`api/new` and the share link is the same query as a GET on `/join`, so a link
cannot come to mean something slightly different from the screen that wrote it.
The server reads it in one place for the same reason. Everything in it is
optional with a default that plays an ordinary game, because a link is text
somebody may have truncated and a dead link is worse than a table with a default
clock on it.

What it does is deal whoever opens it **their own table on this board, set up
exactly like this one**: same seed, seats, market, clock, discard allowance, pace,
bank and log, and the same table name. What it does not do is seat them at your
table, which needs a second person at one game and is still to come. The label
says *Share* rather than *Invite* for exactly that distance.

Two things are deliberately not in the link. The host's name, because whoever
opens it is not them and a link that filled in somebody else's name would be the
wrong kind of helpful; and the host's key, so the table belongs to whoever opened
it and lands on *their* home page.

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
  your move, plus an **increment** credited back for every turn you finish.
  Spend the bank and your turns end as soon as they begin.

  The increment is what makes it a chess clock rather than a countdown. Without
  one a long game is decided by the clock instead of by the board; with one, a
  player who keeps moving keeps playing. Set it to zero for sudden death.
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

**A forced move is written down like any other.** The engine does not know who
asked for it: the same roll deals the same cards to the same seats whether it
was clicked or timed out. The forfeit wrote down the move and not what it
caused, so the log read `Time ran out, rolled 8 for you` and then went straight
to the turn ending. The cards were in people's hands, but a roll that pays
nobody is the one thing a roll cannot do, and from the record the whole table
had been skipped. Production and a robber's theft are now noted on this path
exactly as they are when a person or a bot makes the move.

### A seven has a clock of its own

**The discard is not part of anybody's turn, so it does not spend one.** It
stops the player who rolled from playing, and it asks everyone else for cards on
a turn that is not theirs. Charging it to the turn punished the roller for the
dice; charging it to whoever owes cards would be a second clock on people who
are not playing. So it is neither: **a short fixed window belonging to the seven
itself**, set in the lobby, ten seconds by default, with the turn clock held
while it runs and resumed where it stopped.

It is set even when there is no turn clock. A table can reasonably want the
discard bounded and the turns not. Zero is no limit.

**Run it out and the cards are picked at random**, for everyone still owing
them. This is the one place a clock takes cards out of a hand, and it is
justified the same way every other forfeit is: a discard cannot be declined and
the position is illegal until it is done, so the choice is between choosing
badly for someone and the game stopping on them.

The countdown shows on the discard card and not beside the turn clock, which is
held while it runs: two numbers counting in the same corner where only one is
moving reads as a fault. It is driven by the page's own one-second tick rather
than by the payload, because a wait is exactly the state in which no new payload
arrives, and a number that only moved when the game did would sit still through
the seconds it is counting.

### The clock belongs to the turn

**Whoever's turn it is, and nobody else.** What the passive players do inside
that turn, answering an offer, discarding on a seven, being robbed, is their
business and costs them nothing. If none of them reacts, the turn runs out and
the next player's begins.

**An unanswered offer is a refusal.** An offer left on the table stops the bots,
because they are waiting on an answer. Nothing about that wait is charged to the
person who owes it: it is the turn holder's own allowance draining while they
wait, and when it runs out the offer dies with the turn. That is the one thing a
turn's clock does to a seat that is not holding the turn, and it is what stops
the game standing still for as long as nobody clicks.

**Every offer put to you, not only the ones you could take.** What blocks the
table is being *asked*: a question put to the human is a pending choice, and a
pending choice is what stops the bots. The escape was keyed on what the human
could have accepted, so an offer they could not afford held the turn up and was
then not cleared when the allowance ran out. Reported as "the new turn doesn't
start until the offer is accepted or declined", and it looked exactly like that:
turn nineteen, clock at `0:00`, an offer of one wood for three brick sitting on
a hand of two cards.

This was the other way round for a while: the clock followed whoever owed an
answer, on the reasoning that the wait belongs to whoever is holding everyone up.
It broke two things at once. A player's expired allowance carried off their own
turn and onto the next one, so their clock read a stuck `0:00` through turns that
were not theirs; and enforcement ran on the same request that opened an offer, so
the clock refused every incoming offer before the page had drawn it once. There
is now one field for both questions, `turn_holder`, because they turned out to be
the same question.

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

### Two clocks, two panels

**The turn on the left, the whole game on the right**, each with its caption
underneath. Two different questions, "how long have I got" and "how long has
this taken", and one figure cannot mean both.

**Two panels rather than one panel split by a rule.** They were halves of a
single card for a while, divided by the hairline the bank uses to fence off its
development deck. That line says "these are two parts of one reading", which is
what it means in the bank and not what it means here. Equal widths, in the same
row, the same gap apart as every other pair of panels in that column.

The turn is the accent, because the accent on a clock means *this is running
out*. The game's own time is the same clock in the text's colour: it is not
something to act on, and two accents would say "running out" twice when only one
of them is true.

Both count on the same second, so they can never disagree by a tick.

**An untimed game shows one clock, not two.** With no turn allowance the left
panel is already counting the game up, and the same second printed twice reads
as two clocks that happen to agree. The one that is left takes the full width.

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
`.separator`; `.tabs`; a tooltip; and the decision card as a dialog.

**`.tabs` is shadcn's tabs shape with radio semantics underneath.** A sunken
`--muted` track with the chosen option raised out of it. It replaced a row of
filled buttons, which made every choice look like an action to take rather
than a state the table was already in, and read as four things to press once
there were four such rows stacked.

The semantics are `radiogroup` and `radio`, not `tablist` and `tab`. These
controls set a value; they do not swap a panel, and announcing them as tabs
would promise a tabpanel that is not there. Radio also arrives with the arrow
keys already meaning the right thing: one tab stop for the group, then arrows,
`Home` and `End` between the options.

**The chosen option is the accent, raised out of the track.** shadcn lifts it
onto a paler surface instead, which works where the cards are lighter than the
page. Ours are the other way round, so a paler surface left the choice leaning
on its shadow: `--background` sits one shade off `--muted` and `--card` only a
little further. Colour says it more plainly than a shade does, and it is the
same orange that marks the live thing everywhere else.

**A button is styled by its variant and by nothing else.** If a button needs to
look different, it needs a variant, not a rule keyed to its id.

### Two accessibility rules this pass established

**Unavailable is `aria-disabled`, never the `disabled` attribute.** A disabled
button leaves the tab order and stops emitting pointer events, so its
explanation becomes unreachable by keyboard and by touch, precisely the
audience that most needs it. The click handler guards instead.

**A tab group's explanations sit on its options, not on its heading.** On the
heading a tooltip can only describe whichever option is already selected, which
is the one a reader least needs explained; on the option it answers the
question actually being asked, which is what the other choices would do. Every
value in every group carries one, and the roving tab stop means a keyboard gets
them by arrowing through exactly as a pointer gets them by moving across.

**A tooltip trigger is the size of its own words.** A field label was a
full-width block, so its hover area was the whole row: a tooltip fired from
empty space beside the label and then centred itself over the card rather than
over the thing it explains. Tooltips also prefer to sit below their trigger and
flip above only when there is no room, since a long explanation placed above
covers the rows over the thing being explained.

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

**The five development card faces are drawn**, in `art/dev-*.svg`: brand orange
ground, the name and the number in the wordmark's face, and the drawing in a
three-step ramp of white through very light grey, which is the same lighting the
isometric pieces use. Each is built from art already in `art/`, so the card and
the thing it does are the same object: the militia carries the robber it moves,
road building carries two roads at the board's own angle, and invention and
monopoly carry resource cards drawn the way the game draws one, a face and a
disc. The victory point carries no drawing at all, just `VP` and `+1`.

One number per card, in the same place: `+1` militia, `+2` roads, `+2`
resources, `ALL` of one resource, `+1` point. The five drawings **stand on the
same line**, which was measured rather than eyed: they bottom out between y 90
and 93.3 on a 132-unit card, clear of the number below.

Bottoms rather than centres, and that was a correction. Centring all five in one
band works only while they are about the same height, and the robber is 59 units
tall against roughly 46 for the rest, so a shared centre put its base on top of
the `+1`. Lining up the bases instead lets a tall drawing be tall.

The two roads on road building are offset **across** the road's own axis rather
than along it. End to end they read as one long road with a joint in it, which
is not what `+2` means. They sit close enough together to read as a single mark,
and the pair is moved symmetrically about its midpoint so tightening the gap
does not walk the drawing off centre.

**The two awards are the same face**, in `art/award-*.svg`: longest road and
largest militia, `+2` each (R-10.1, R-10.8). They reuse the militia's robber and
road building's road, arranged to mean "most" and "longest": three robbers with
the middle one forward, and three roads end to end.

End to end is measured off the road's own axis rather than its bounding box.
Head to tail the drawing runs `(68.40, 39.49)` at scale 1, which is thirty
degrees exactly, while its box is `76.81` by `54.05` because a box has to hold
the mitred ends too. Stepping by the box left a gap at every joint and the run
read as a dashed line rather than as a road.

`art/card-faces.py` is where all seven come from. The template is the source and
the SVGs are its output, so a change made to one file rather than to the script
is undone by the next run.

Needed, and not yet drawn: five resource objects (brick, wood, wool, wheat,
ore), the robber as a piece rather than a marker, and tokens for the longest
road and the largest militia. These come from the same hand as the existing
pieces; the interface is built against placeholders in the meantime and swapping
them is a file drop into `art/`.

---

## 12. Analytics

Every game has an **address**, and its analytics are a sibling of it:
`/<id>/` is the board, `/<id>/analytics` is the report. The root is not a game,
it is where you go to get one: it mints a table and sends you to its address.

**A game is a seed, a table and a list of steps**, written to a file after
every move. The engine is deterministic, so those few hundred bytes are the
whole game: replaying them rebuilds the position down to the next random number
and the account of it down to the log line. Nothing derived is stored, because
everything derived is regenerable (H-7), and a metric that changes is
recomputed rather than migrated.

**A step is a move or a refusal.** Turning an offer down changes no position, so
the engine has no action for it and is right not to, but it is the answer the
whole table was waiting on and the record would tell a different story without
it. `carranta-record` draws the same distinction.

**When each step landed rides beside the moves, not inside them.** File format
version 2 adds `at` lines, a list of milliseconds from the deal, one per step.
Beside rather than inside for the reason the moves are what they are: a step is
enough to rebuild the game on its own, and *when* it happened is something known
about the step. So a version 1 file is still a whole game that simply has no
clock, and its times read as empty rather than as zero, because "nobody knows"
and "it took no time" are different answers and only one of them is true. A
clock that has fallen out of step with the moves is refused outright: the wrong
seconds on the wrong turns is a wrong answer, where none at all is a missing
one.

**A game nobody has heard of is a 404**, not a fresh board. An address that
silently becomes a different game is worse than a dead link. Games that are not
the live one are read-only: a POST to one is refused rather than quietly
driving whatever is being played now.

**A game nobody has moved in is not written down.** Every visit to `/` deals a
table, so writing one at that point meant a file for every time the page was
opened and closed again, each of them a seed and nothing else. Those are not
abandoned games, they are games that never started, and they were dividing
every figure the analytics computed across the store. A game abandoned halfway
is still kept: it happened. The first move is what makes a file.

**The corpus is finished games only**, which is a filter this page applies
rather than one `carranta-analytics` applies for itself: that crate counts what
it is given and reports `finished` beside it, rightly, since a half-played game
is still evidence about the dice. It is not evidence about anything on the
report. A game nobody won has no finishing order, so it says nothing about
whether going first is worth anything and only enlarges the denominator, and
its dice are a handful of rolls whose deviation from fair is enormous by
construction, so placing a full game against it is not a comparison. The
ratings already refused unfinished games (`Pool::record`), for the same reason.

### How the report is dressed

The page borrows **shadcn/ui's design system and none of its code**. shadcn is
not a library you link to: it is a CLI that copies React and TypeScript source
into a project, and every component of it assumes React, Tailwind, a Radix
primitive or two and a bundler. Its charts are Recharts, another React
dependency. This workspace has no Node toolchain and no third-party crate, and
`./play` is `cargo build --release` and run, with every asset compiled into one
binary you can copy to a machine and execute. Taking the package would cost all
of that to replace hand-written CSS that was already doing the job, and would
ship a JavaScript runtime to a page that deliberately carries no script.

What was worth taking is the system underneath the React, which is MIT and is
mostly a set of good decisions:

- **The token vocabulary.** `--background`, `--foreground`, `--card`, `--muted`,
  `--muted-foreground`, `--border`, `--primary`. The names say what a colour is
  *for*, where the old `--dim` only said what it looked like. The values are
  still Carranta's ink and paper, so the report belongs to the game.
- **One radius, everything derived.** `--radius` with `sm`/`md`/`lg`/`xl`
  computed off it, which is what keeps a badge, a table box and a card looking
  like one family.
- **The card.** Border, generous padding, one shallow shadow. The two-layer
  lift the page had before read as a modal floating over something, and a
  report is not floating over anything.
- **Title and description before content.** A label tells you a section exists;
  a description tells you whether to read it. This replaced the uppercase
  micro-label headings.
- **Tables in their own bordered box**, headers muted, medium weight and
  sentence-cased rather than shouted, rows that answer to the pointer. Numeric
  columns stay right-aligned: that is a column of figures whatever the design
  language says.
- **The muted-foreground habit** for anything secondary.

### Explanations are tooltips, and totals are a foot

**A card is a title and a table.** Every rule that used to sit in a paragraph
under a table is now on the thing it is about: a column's rule on the column
header, a card's on its own heading, both under a dotted underline so a reader
can see there is something to hover. A note under a table is read through to
reach the figures, or not read at all; a tooltip is one hover from the reader
who wants it and invisible to the one who does not.

**The page draws every one of those tooltips itself.** The browser's own box is
a grey system rectangle in a system font at a size the page never uses: the one
element on a carefully set report that belongs to no design at all. Ours is the
card's ink on the card's paper at the family radius, and it is the same box
everywhere, over a table or over a drawing. It hangs off `data-tip` rather than
`title` because the two cannot coexist, and the cost is honest: a `title` is
reachable by keyboard and this is not, which is why nothing on the page lives
*only* in a tooltip.

Three details, each of them a bug found by measuring rather than by looking:

- **It opens downwards, and a table stops clipping while it is open.** A header
  is the thing most often asked about and there is always table under it. But a
  table sits in a box that scrolls sideways, and a box that scrolls on one axis
  clips on the other, so a one-row table would cut its own explanation in half.
  `:has([data-tip]:hover)` turns the clipping off for exactly as long as a
  tooltip is up.
- **Near the right edge it hangs from its own right.** The box is wider than a
  number column, so the last three columns of any row open inwards.
- **A hidden tooltip is out of layout, not merely invisible.** A hidden box is
  still a box: absolutely positioned inside a table that scrolls, it stretched
  that table's scrollable area, and the one-row table under the dice histogram
  came out with a scrollbar and its own header scrolled out of sight. `display:
  none` until hovered costs the fade and fixes it.
- **A tooltip in a drawing is laid over the drawing, not drawn in it.** SVG has
  no pseudo-elements and no text that wraps, so the box would have to be a
  `foreignObject`, and a `foreignObject` with *any* SVG geometry painted after
  it is dropped behind the whole drawing by the browser. That is always the case
  here, since every later shape is geometry. So the drawings carry a layer of
  page HTML over them, positioned as a percentage of the drawing's own box so it
  holds at any width, and the tooltip comes out at the page's own text size
  instead of at whatever the drawing was scaled to.

  Tying one of those to its shape is the one place this page pays for having no
  script: CSS can ask whether a drawing holds a hovered shape but cannot be told
  *which*, so each drawing writes a rule per shape beside itself. About forty in
  a typical game, and the alternative is a script.

  The per-turn slots on the production chart went the same way and are now plain
  HTML columns over the chart, carrying the ordinary `data-tip` box and drawing
  the guide line as their own left edge.

  The **names and place badges in the drawings** followed the tooltips out for a
  plainer reason: inside the picture they were scaled with it, so a badge came
  out a different pill from the badge in every table. A badge that changes shape
  between two places on one page is the one thing a badge must not do. Over the
  top it is the same markup at the same size, down to the line height, which is
  what a pill takes its height from.

There is no subtitle anywhere. A sentence describing a card the reader is
already looking at is a sentence they read to learn nothing, and a sentence
carrying a figure is a figure that belongs in a table, so the three that carried
figures were folded into theirs: the dice card gained a deviation, percentile
and games-compared row beneath its histogram, the robber card a moved and
found-nothing row beneath its grid, and the corpus card games and won columns
beside its win rate. Figures that belong to nobody's row get their own small
table rather than a sentence over the one above. The only paragraphs left
anywhere are answers rather than explanations, like "Nobody finished a turn in
this game".

**The seat selector wears the seat's colour.** Each pill carries the seat's mark
left of the name, and the pill fills with that colour once it is the view on
screen. Picking Ines and then reading four teal lines is a moment of doubt the
control can spend a colour to remove. The class doing it is `m0`..`m3` rather
than the `s0`..`s3` the marks use: those set a background on anything wearing
them, and reusing them painted every pill in the row rather than the one picked.

**A seat wears its colour and its finishing place beside its name, everywhere
the page names anybody.** The colour is the mark the board plays in, always
immediately left of the name, so a row can be found by colour rather than by
reading down the names. The place is a badge on the right, first place in the
colour the win had and the rest quiet. A badge on the winner alone answered
"where did they come" for one player and left the other three to be worked out
from a column of points.

**Nothing is written as nothing.** A cell with no value in it is blank. There is
no dot standing in for the absence: the blank already says it, and a page of
marks meaning "not applicable" reads as a page with something in every cell.

**A totals row wherever a column has a total.** Turns, moves, time, cards
bought, offers made, what the board paid. Not where a column has none: a
maximum, a z-score, a rate and a diversity count do not add up, and a row that
totalled them would be read as though they did, so those cells are a dot. The
ratings table totals only the change, which very nearly cancels, because an
update is a redistribution and the before and after columns are positions rather
than quantities.

### What the report says, and what it refuses to say

The whole of `carranta-analytics` reads a game record, and this is that crate
put in front of a person. §10's one rule shapes the writing throughout: **small
n makes p-values invalid, large n makes them uninformative**, so every figure
is paired with an effect size.

- **Result**, broken into what scored. The five things that do (R-11.3),
  each as how many were held with what they were worth in brackets, so the
  bracketed figures add across to the total: a settlement one, a city two, each
  tile two, each victory point card one, and a road nothing however many there
  are. Counted off the final position rather than off what was built, which is
  a different number, since a settlement upgraded to a city stopped being a
  settlement and was still built. The total is the true one, hidden cards
  included, which is not what the table could see while it was playing.

  Under the table, the score turn by turn, one stepped line a seat. A result
  says who won and by how much; it cannot say whether the game was ever close,
  and a seat that led for a hundred turns and lost reads exactly like a seat that
  was never in it. The lines are the true score too, so each one ends on that
  seat's points column, and a test holds the two together. They step rather than
  slope, because a score holds until something changes it, and they go *down*
  where the longest road or the largest militia changed hands.

  Two lines a seat, though: the true score solid, and **the score the rest of the
  table could see** dotted beneath it, which is the same total less the hidden
  victory point cards. They part the moment a card is drawn and never rejoin, and
  the gap is the whole tension of an endgame. In the demo game the winner crosses
  the finish line at ten while their visible line stops at eight, so the table was
  playing against a seat two points further along than it knew.

  Under the chart, **what was happening while those lines moved**: a lane a seat,
  a mark a thing, on the same turn axis. Every chart on this page provokes the
  same question and none of them could answer it, since a line steps up around
  turn ninety and nothing says why. Settlements, cities, cards bought, and the two
  tiles arriving, which is everything that changes a score or an engine. Shapes
  rather than colours for the four kinds, since the colour is already saying which
  seat: a filled square is a settlement, a bigger one a city, a ring is a card
  bought, and a diamond riding just above the lane is a tile changing hands,
  lifted off the line because it lands on the same turn as a building often enough
  that the two marks sat on top of each other.

  No names on the lanes. They needed an inset to fit, the inset pushed the strip's
  turn axis out of step with the chart above, and being read against that chart is
  the whole reason the strip exists; the legend between the two already says which
  colour is whom. A check measures both drawings and fails if their boxes are not
  identical.

  The class names there are prefixed `beat-`, which is not fussiness: `mark` is the
  header's wordmark and `tile` is the opening card's hex, and an SVG rect takes
  CSS `width` and `height` over its own attributes, so reusing `tile` flattened
  every diamond on the strip to nothing at all. That is the second time a class
  collision has silently eaten a drawing on this page.

  Under both, **the game as a race**, which is what the chart asks to be read for
  and cannot state: the turn each seat reached half the target, the turn they came
  within two points of it, how many turns they led outright, and the last turn
  they led. A tie is nobody's lead, since two seats level are both ahead of the
  others and neither is ahead of the other, so the foot carries how many turns had
  a tie at the top. The foot also carries the length of the endgame: the first seat
  to come within two points, and how many turns the rest of the table then had to
  stop them. In the demo game the winner led for forty-seven percent of the turns
  and came in reach with twenty to go, and the seat that finished third last led on
  turn thirty-seven.

  The **finish line** is drawn across at ten points, because "was it close" is
  read against the finish rather than against the top of the paper. Both this
  chart and the engine chart carry legends now, and the legends are also controls:
  clicking a name takes that seat's lines off the chart, the same way the
  production card's does.
- **Turns**, per seat: turns, time, and share of the game's time. A turn is
  what falls between two ends of turn, and it counts everything that landed
  inside it, the turn holder's or not, since a discard, a robbery and an
  accepted offer all happen in somebody's turn; the time is wall-clock on the
  same basis. Setup placements are left out: they come before anybody has a turn
  to take. Time and share appear only when the file has a clock in it, so a game
  saved before version 2 shows its turn counts and says nothing about time.
  Beneath it, **where the clock went by kind of decision**: setup, rolling,
  building, development cards, trading, the robber, discarding, ending a turn. The
  file has stamped every move since format 2 and the page only ever added them up
  per seat, which left "a hundred and fifty turns of twelve seconds" with no way
  to ask what the twelve seconds were spent on. Time is charged to the move that
  *ends* the wait rather than the one before it, since the gap between two stamps
  is somebody deciding what to do next and the move that lands is what they
  decided.

  The decision counts carry the card on their own: two hundred and forty-nine
  trading decisions against forty-five builds says what a game was made of. A
  table of bots decides a whole game inside a few milliseconds, which is finer
  than the clock records, so for those games the time columns are blank rather
  than a column of noughts inviting somebody to read them as findings.
- **Ratings**, which is the section this page exists for. Before, after, the
  change, and the total games each player has been rated on here, this one
  included. No totals row: before and after are positions rather than
  quantities, four players' games played is not a number of anything, and the
  changes do not cancel exactly enough for their sum to be worth printing. A Weng-Lin Plackett-Luce update over
  the whole finishing order rather than just the winner (A-1). The figure shown
  is the conservative estimate, three standard deviations below the mean, and
  beside it the games each player had behind them, because that is how much the
  number is worth believing. Ratings are computed by replaying every recorded
  game in order and reading the pool either side of this one: a rating is a
  function of everything before it, so what a result did cannot be worked out
  from the result alone.
- **Dice**, as a chart of what was rolled with the fair-pair expectation marked
  across each bar, over a table of both figures. The chart is a row of that same
  table, so a bar and its column are aligned by the table rather than by a
  number kept in step by hand, and bars and marks share one axis, the tallest of
  either. **No p-value**, deliberately (§10.1): across enough games one in twenty
  clears p<0.05 by construction, and those are precisely the games somebody
  screenshots as proof of rigging.

  Beneath the histogram, four figures, and three of them were fixed after a
  review of the statistics rather than of the code:

  - **Out of place**, how many rolls landed on a different number than a fair
    spread would put them on. Total variation distance times the rolls, which is
    exactly the count that would have to move to make the histogram fair. Bits are
    the right thing to rank a corpus by and nobody has ever looked at a game and
    thought in bits; this is the same deviation in a unit somebody can picture.
  - **Deviation**, the KL divergence in bits, now **less the bias a finite sample
    puts into it**. A plug-in KL estimate is biased upward by about `(k-1)/(2n)`
    nats, which at a hundred and forty rolls is 0.05 bits: the same order as the
    deviations being ranked, and always in the direction of "this game was
    unlucky". Since the corpus holds games of every length, the raw figure ranked
    short games as unluckier than long ones with identical dice. Floored at
    nought, so a fair game now reads nought rather than a positive floor.
  - **Standing**, where the game sits among every finished game, most deviant
    first, as a place rather than a percentile while the corpus is small. A
    percentile of six games moves twenty points when a seventh is played, and
    printing it to the percent claimed a resolution the corpus did not have. Past
    a score of games it becomes a percentile, which carries more than a place
    does. Blank until there is a second game, because a percentile of one game is
    not a percentile.
  - **Games compared**, so the figure beside it can be discounted properly.

  Sevens are *not* given a row of their own: the histogram's seven column already
  carries the count and the expectation, and a second copy of two numbers is not
  a finding.
- **Production**, a ledger. Every card that reached a hand or left it, by what
  moved it: production, invention, monopoly, stolen and traded in against built,
  discarded, robbed, monopolised and traded out. Read down, what came in less
  what went out is what was still in hand at the end, and `Ledger::balances`
  asserts exactly that. It is read off the hands rather than off the rules, by
  applying each move and comparing the hands either side of it, so a card that
  moved is counted whether or not the code knows why; only the *reason* comes
  from the action, and that match is exhaustive, so a new action cannot be added
  without deciding where its cards belong. Gross rather than net, per resource:
  two wheat for one ore is one card in and two out, and a net hand size would
  call it one card out and lose both figures. The §10.2 decomposition is not on
  this card: expected production is a counterfactual rather than a card that
  moved, and it belongs with the per-turn view rather than beside a ledger.
  The ledger closes with two rows that are not card flows and belong to the same
  hand: the biggest hand ever held, and **turns ended holding more than seven
  cards**. The discarded row says what a seven cost; this says how long the seat
  was exposed to it. Discarding nothing all game is careful play or a quiet table,
  and only the two rows together say which.

  Beneath the ledger, **which cards were thrown away**. The rule takes half a hand
  on a seven (R-6.2) and the player picks which half, so a discard is a decision and
  one total cannot show it. The per-seat totals here are the ledger's discarded row
  reached from the other side, which makes the two a check on each other, and the
  composition is the new part: in the demo game the seat with four ore-heavy cities
  threw away fourteen ore.
- **Production per turn**, as a chart. Solid is what the board paid, dotted in
  the same colour is what the pips through the buildings standing at each roll
  owed at fair odds, and both are running totals, so each line only climbs and
  the gap between a pair is everything that has happened to that seat so far. A
  per-turn figure would be nearly all zeroes with occasional spikes: most turns
  pay a given player nothing. The expectation ignores the robber (§10.2's
  `e_raw`), so a seat under blockade watches its solid line fall away from its
  dotted one, which is what a blockade costs, drawn.

  Above it, a switch: everybody, or one seat at a time drawn a resource at a
  time, which is the only way to see *which* card a placement was short of. The
  switch is five radio inputs and a sibling selector, so the page still carries
  no script; every view is drawn into the page and CSS decides which is visible,
  because five charts of a few hundred points are a smaller thing to ship than a
  script that would build one. Per-player URLs are a later thing, for when
  profiles and profile history need linking.

  The legend below each chart is centred and is also its control: a checkbox a
  curve, so clicking a name takes that pair of lines off the chart, and the name
  greys with its swatch gone hollow to say so. The rule is positional rather
  than by id, so five lines of CSS cover the same curve in every view. The turn
  axis is labelled along its length at a step somebody would have chosen, with
  the last turn always named, since where the game ended is the turn a reader
  looks for and an even step usually misses it. And a slot per turn sits over
  the whole plot carrying that turn's figures for every curve, with a guide
  under the pointer.

  The last point is the end of the game rather than the end of the last
  completed turn: the winning turn never ends, and dropping its cards would put
  the chart out of step with the ledger above it. The opening settlement's
  payout counts on **both** lines, because it is a certainty rather than a
  wager: it pays what it touches, once, with no dice involved, so it is owed
  exactly what it paid. Counting it on one line only offset every seat by a few
  cards for the whole game.

  The everybody table carries a **deviation** column, how far a seat's total ran
  over or under what was expected as a share of it, and under it the same
  question asked four times across the game. When a seat was starved matters:
  cards missing early delay everything they would have bought, and the same
  shortfall at the end costs one purchase. A quarter is what the running totals
  grew by across it, and each cell carries the cards it was measured over in
  brackets, since a third of six is one bad roll and a third of sixty is a game
  being lost. The last column is **total**, the four quarters together.

  Under each chart, its own table, so the figures change with the view. Every
  seat has one: production per resource with the expectation in brackets, and a
  foot row for the board. One seat has another shape, a resource a row:
  production, expected, the difference between them, and what share of
  everything they collected each resource was. The two answer different
  questions, which is why they are not the same table transposed: the first asks
  who did better, the second asks what this seat was living on and what it was
  short of.

  The quarters table appears in a seat's own view too, a resource a row rather
  than a seat: a seat that lost its ore in the third quarter was not short of
  ore all game, and the whole-game figures above cannot tell the two apart. It
  is the same builder in both places, given rows and a way to read a running
  total off them, which is also why the two cannot drift.

  The bracketed figure is called **expected**, not owed, everywhere it is shown.
- **Militia**, as a sankey: thieves down the left, victims down the right, a
  ribbon between each pair as thick as the cards that moved along it. Laid out
  on the server like everything else here, since every position is a fraction of
  a total known the moment the game ends. The cards themselves are in the ledger
  above; this is who took them from whom, which a per-player column cannot say.
  With it, the robber's own counts: times moved, and robberies that found an
  empty hand.

  Then **the robber as a blockade rather than as a thief**, which is its quieter
  and often larger job: turns each seat ended with the piece sitting on a hex they
  had built on, that as a share of the game, and what it cost in cards, taken from
  the deviation card so the two cannot disagree about the same robber. A robber
  parked on the wheat 8 for thirty turns decides a game without stealing a single
  card, and the page had no way to say so. Under it, the hexes it sat on longest,
  named the way a player names them: the resource and the number, because "the
  wheat 8" is a thing somebody remembers and "hex 11" is not. In the demo game the
  winner was blockaded for fifty-five percent of the turns, the most of anybody,
  and won anyway.
- **Trades**, as a chord: every party round a circle, each arc as long as their
  trades, and **one ribbon per trade** rather than one per pair, so a thick band
  is a run of deals you can count rather than a number you have to hover to
  read. Each ribbon carries the trade it is: which turn, who gave what and took
  what, and against which counter. The circle and its table sit side by side
  while there is width for them, so the card is not a drawing floating in a
  field of nothing. The bank and the ports are parties too, since a trade with the
  supply is still a trade and leaving it out would draw a market smaller than
  the one played. A chord rather than a sankey because trading is symmetric:
  there is no side a trade goes from, and drawing one would invent a direction
  the game does not have. Which counter a supply trade used is read off the
  price rather than off the ports a seat owns: four cards for one is the bank,
  three or two is a port. Beneath it, the counts, where a completed trade is
  counted for both sides so that column totals to twice the trades; the circle
  counts each one once.

  And beneath that, **what the trading was worth** rather than how much of it
  there was: cards given, cards taken, the difference, the price in cards handed
  over for each card taken back, and net cards handed to the seat that won. The
  counts above cannot say whether a seat came out of eleven trades ahead, what it
  paid, or which seat it spent the game feeding. A deal is recorded once, from the
  offering side, so every figure here is read from both sides or every
  counterparty would look as though it had never traded. The given and taken
  columns are *not* required to match, and the gap is the interesting part: a
  trade between two people moves cards sideways, a trade with the supply takes
  them out of the game, and the difference is what the table paid the bank and the
  ports for the privilege.

  Last, **what was in the offers** rather than how many there were. Three counts,
  offered and withdrawn and turned down, cannot tell a seat nobody would deal with
  from a seat asking two cards for one: different problems, different answers, same
  three counts. So the ask: cards wanted across every offer, cards put up for them,
  the ratio, and how many of those offers anybody took. The demo games answer their
  own question here, and the answer was a bug. Every seat asked about 2.4 cards for
  1.3, an ask of 1.86 to one, and **not one offer between players was ever
  accepted** in a hundred and fifty-four of them. That reading started the market
  fix described below; on the games played since, the ask is nearer 1.6 and about
  half the offers are taken.

  Last of all, **what the offers were asking for**, resource by resource: cards
  wanted less cards put up, so positive is a seat trying to buy that card and
  negative a seat trying to sell it. This is the honest answer to a question the
  backlog asked differently. "Who did each seat aim its offers at" cannot be asked
  of these games at all: the generator only ever makes open offers, on purpose,
  since addressing one multiplies the action space by the number of opponents for
  nothing, so the count of addressed offers is nought for every seat and is kept
  only because a human client may still make one. What *is* in an offer is the
  useful question, and it reads across to the rest of the page: the seat that spent
  the game trying to sell ore is the same seat that threw fourteen ore away to
  sevens and had four cities making it.

  **This card found a bug in the game, twice over.** Its first reading was that not
  one offer in a hundred and fifty-four was ever accepted, and the natural
  conclusion was that an ask of 1.86 to one was too greedy for anybody. That was
  half of it: the bot's acceptance rule was too strict, taking a deal only when the
  deal left it further ahead of the table than the seat offering it, which the
  offerer had chosen not to be the case. But one in seven offers *was* taken in the
  bot's own driver and none at all in the recorded games, and that gap was the
  second bug: the path that plays a game out never settled the market, so no offer
  in any recorded game was ever put to anybody. The zero in the turned-down column
  was the tell, and nothing but a card showing offers and answers side by side would
  have shown it. Both are fixed, and the games on this server now carry sixty to
  eighty player trades each.
- **Board**, what this board dealt against what an average one deals. The discs
  are a fixed set laid on a fixed set of hexes, so the average is not a
  simulation: it is the mean pips of a disc times the hexes a resource has.
  Every disc lands somewhere, so the pips always add to the same total and the
  difference column cancels exactly, which makes the card a pure redistribution
  and the question it answers "which resource did the deal favour". The same
  question is then asked of the coast, over the intersections each port kind can
  be built on.

  The coast table's expectation is **not** the same for every port, and that is
  geometry rather than chance: a port spot touches one or two land hexes
  depending on where it sits, so a port whose two spots reach three hexes is
  expected more pips than one reaching two. The layout is the same on every
  board, so only the difference column is luck. The **a hex** column divides
  that out and is the figure to compare ports on, against the board's mean of
  3.2 pips a hex.

  A third table asks **what kind of board this was**, which the pip tables cannot:
  two boards can owe every resource the same pips and play completely differently if
  one has its ore spread around the island and the other has all of it in a corner.
  Neighbouring hexes making the same resource, against what a random deal of the
  same tiles would be expected to produce; whether a six was dealt beside an eight,
  which some rule sets forbid and this one does not; the best intersection on the
  board with its numbers; how many intersections were worth planning a game around;
  and the mean intersection, which is what the best one has to be read against.

  The clumping expectation is exact rather than simulated, which the shape of the
  problem allows: the adjacency graph is fixed, so its forty-two neighbouring pairs
  can be counted, and for a shuffled set of tiles the chance any given pair matches
  is the chance two tiles drawn without replacement are the same terrain. The demo
  board comes out at four against an expected 5.9, so it was slightly less clumped
  than average. A row is named by its disc alone, in the port's own colour, as
  the board and the opening name it; the resource beside it was the same thing
  said twice, and it is on the disc for anybody still learning the colours.
- **Development cards**, each column how many of that card were drawn with how
  many were played in brackets. The victory point column has no brackets: a
  victory point card is never played, it counts from the moment it is drawn
  (R-9.11), so a bracketed nought on every one of them answered nothing. The two differ by what was still in hand at the
  end: a card is drawn once and then either played or held, and a played card
  never goes back to the deck (R-8.10). The foot row is therefore the deck's own
  composition, which is a standing check on the whole table.

  Under it, **how long each kind waited in hand**. The counts cannot say when a card
  was played, and a militia played the turn after it was drawn is a different
  decision from one held for forty turns: the first is a seven happening to
  somebody, the second is a player waiting until the robber was worth moving. A play
  is matched to the oldest unplayed card of its kind, since cards of a kind are
  interchangeable and any other rule would be arbitrary in the same way while
  reading worse. Cards still in hand at the end get their own column, because a card
  held to the end is a decision too and a mean over played cards alone would quietly
  drop it.

  The turns are table turns rather than the holder's own, so in a four-player game
  four is a card played at the first opportunity the rules allow (R-9.4). Every
  militia in the demo game waited exactly four, which says the bot plays them the
  moment it may; the two Road Building cards were never played at all and sat in hand
  for a hundred and five turns.
- **Building**, where each seat's cards went and what stopped them spending more.
  The ledger says a seat spent forty-six cards on building; roads, settlements,
  cities and development cards are four different decisions, and one number for
  all four says nothing about which game the seat was playing. Counts with the
  cards they cost in brackets, and the spent column is the ledger's built row
  reached by a second route, so if the two ever disagree one of them is wrong.

  Prices are read off the hand rather than from the rules table, so a road from a
  Road Building card costs what it really cost, which is nothing. The opening's two
  settlements are not counted here at all: they were placed rather than paid for,
  so a blank in the settlements column means a seat that never built beyond its
  opening, which is a strategy and not a gap.

  Two columns are not spending. **Longest chain** is the road network each seat
  finished with, which is what the road tile is contested on and the only thing a
  seat builds that nothing else on the page shows unless they won it; it can fall
  as well as rise, since a settlement built through the middle of a road cuts it in
  two. **Stuck** is turns that ended with the seat able to afford a settlement and
  nowhere legal to put one, which is not thrift: those cards cannot be spent and
  sit in the hand waiting for a seven to take half of them. It is a real way to
  lose a game and an invisible one, since nothing in a result or a ledger leaves a
  mark where a player wanted to build and could not.

  Two tables under it. **What the roads did**, because roads are the one thing on
  the board with no score and no production, so a count of them says nothing at all:
  two seats can build eight each and one of them has opened four places to live
  while the other built into a wall. A road is worth the difference it makes, so the
  difference is measured either side of the move that built it: spots it opened,
  whether it lengthened the longest chain, and neither. The first two overlap,
  because a road can do both, and only "neither" is exclusive. In the demo game the
  seat that finished third built seven roads and opened nothing with any of them,
  and spent thirteen percent of the game unable to place a settlement, which is the
  same fact told twice.

  And **three walls**, because being stuck is three different problems: a settlement
  with nowhere legal to stand, a city with no settlement of your own left to
  upgrade, a road with nowhere to go or none left in the box. Able to pay is half of
  each, since a board with nowhere to build costs nothing to a seat that could not
  have paid anyway.
- **Opening**. The pips and resources columns are one: a row per resource, a
  hex per dot, so the cell says how much production the placement bought *and*
  what it bought, and a resource nobody can produce reads as the gap it is.
  Beside it the same pips as cards a turn, which is the unit somebody plays in,
  a pip being a thirty-sixth of a card. Then the numbers the placement sits on,
  drawn as the board draws them with six and eight in the board's red. Then
  **coverage**: the chance a roll pays the placement anything at all, being the
  distinct numbers it touches weighted by how often each comes up. Pips say how
  much an opening collects; coverage says how often, and only the two together
  tell eight pips on one number from eight spread over three. A test finds real
  pairs of openings with equal pips and unequal coverage. Then the ports, at the
  rate they trade.

  Each placement's own total closes the pips and per-turn columns, ruled off like
  a table's foot and on the same line as each other: twenty-two pips, and the
  0.61 cards a turn that is the same fact in the unit somebody plays in. The
  pips total is written as the figure rather than as more hexes, since a row of
  twenty-two tiles says less than the number does.

  No totals *row*, though: four openings' pips added together is a number about
  the board rather than about anybody, and drawn as fifty hexes it is a picture
  of nothing. Biggest hand left this card for the ledger, which is the card
  about hands; it was never an opening figure.
- **Deviation**, what became of each seat's expectation on the way to their hand
  (§10.2). One number for the gap was three causes in a coat: the dice, which
  are chance; the robber, which is the rest of the table choosing to sit on your
  hexes; and the supply, which is a rule (R-5.6). They have different answers,
  respectively shrug, play differently, and nothing at all, so they are four
  columns adding across to what arrived rather than one figure standing for all
  of them. The engine computed this decomposition from the first day and the page
  never showed it, which was the largest gap on the page.

  The dice column carries its own standard deviation in brackets, and that figure
  is exact rather than estimated: production on one roll has a known distribution
  over eleven outcomes and rolls are independent, so the variances add even as
  the buildings change under them. In the demo game every seat lands inside one
  sd of fair while their deviation percentages read minus fifteen, which is the
  card earning its place: those shortfalls were the robber, not the dice.

  Rolls only, which is why the arrived column sits a card or two under the
  ledger's production row. The opening settlements paid before anybody rolled,
  and a payout with no dice in it belongs in neither the expectation nor the luck.
- **Engine**, what one roll was worth to each seat at the end of every turn, in
  cards, and how fast that grew. The engine rather than the earnings: the
  buildings standing at the time, read off the board, so no run of dice and no
  turn that happened to hold no roll can move it. The cards that actually
  arrived are this plus the dice, and the production card is where they live.

  The rating rests on an assumption, stated because it is one: **an economy that
  compounds beats one that is merely large**. Cards a turn buy buildings,
  buildings buy more cards a turn, and a seat whose rate keeps climbing finishes
  with an engine the others cannot catch. So the column to read is the slope
  rather than the size, and it is fitted through the log of the engine, which is
  what makes it a rate: two percent a turn is a doubling in thirty-five.

  **Two accounts of the same numbers are fitted, and the shape column says which
  one won.** Over the range one game covers, one and a half to two and a half
  times the opening, the log of a straight ramp is very nearly straight too, so a
  good log fit *on its own* cannot tell compounding from steady accretion, which
  is the exact claim the card exists to make. So a straight line through the
  engine is fitted as well, and the wider fit wins by a clear margin or the
  answer is "steady". In the demo game both engines that grew come out steady,
  not compounding, and the growth figure greys out entirely for the seat whose
  engine barely moved. Calling that pair compounding would have been the whole
  error this column exists to prevent.

  A doubling time longer than the game is arithmetic rather than a finding: at a
  tenth of a percent a turn the answer is six hundred turns and the game lasted a
  hundred and fifty, so the column is blank past the end of the game.

  Beside it the **fit**, from nought to one, and it is the honest half of the
  pair. Compounding here is bounded at both ends: the opening is a standing
  start, the pieces run out, and the game stops at ten points. A seat that built
  steadily comes out around .9 and the growth figure means what it says; a seat
  that built twice and stopped comes out near .2, and the growth figure is
  average steepness rather than a law it was obeying. No p-value anywhere near
  it (§10.1).

  Two details the first draft got wrong, both of them found by a test rather
  than by looking. The opening settlement's payout is a lump and not a rate, so
  the first turn is left out of the fit; left in, it sat five times above the
  rest of the game and turned every engine into a decaying one. And the last
  turn is left out at the other end, because a game ends the moment somebody
  reaches the target, so the winning turn is a part turn.
- **Coverage**, the chance a roll pays a seat anything at all, turn by turn.
  Solid is what they collected on, dotted is what their buildings reach with the
  robber ignored, and the gap is what a blockade cost in *rolls* rather than in
  cards. Every number their buildings reach, weighted by how often the dice make
  it and counted once however many buildings sit on it: a number that pays twice
  still comes up as often as it comes up. The scale is in quarters of certain, as
  far up as the game reached, so a line's height means something on its own
  without a quarter of the card being paper.

  The table's average is the figure to compare seats on, since coverage that was
  high for ten turns and low for a hundred was low. **A payout** is the engine
  divided by the coverage: cards on a roll that pays. Two seats can be owed the
  same cards a turn and collect them in halves or in threes, and that column is
  which of the two they were doing.

  Beneath the table, **coverage a resource at a time**, which is the builder's
  version of the question. The column above answers "does a roll pay me anything",
  which is what a trader wants to know; a settlement costs a brick, a wood, a wool
  and a wheat, and a seat covered on four numbers that all make wool is not covered
  for anything it is trying to build. The five do not add to the coverage above and
  are not meant to: one roll can pay two resources and is counted under both. A
  blank is a resource this seat could only ever get by trading for it.

  Below it, the two halves of an economy plotted against each other: how often
  it pays across, how much a roll is worth up, a point a quarter, joined so a
  seat is a path with a direction rather than a dot. The direction is the whole
  point. Up and to the right is an engine getting bigger *and* broader, which is
  what a fifth settlement on new numbers buys. Straight up is more of the same
  numbers: bigger payouts, no more often, an economy that spends most of its
  turns with nothing to trade. A point a turn instead of a point a quarter would
  be a cloud of a hundred and fifty dots a seat with no direction visible in it.
  The stops grow and darken through the game, so the path reads as a path without
  hovering it.

Seat win rates were on this page and are not any more. They are a claim about
many games, and a report on one game is the wrong place to make it; they belong
on a page that reads the whole store, which is the cumulative statistics work
still to come. That page's brief, and everything else the analytics do not answer
yet, is written down in `analytics-backlog.md` rather than carried around in
somebody's head.

**Each seat is a rated player, not "the bot".** The three heuristics are the
same player underneath, so their ratings should converge on each other, but a
ranking cannot list one player three times and per-seat identities are also
what make the seat-order balance mean anything.

**The report is a document, not an application.** Everything on it settled the
moment the game ended, so it is rendered on the server and carries no script.

**`--demo N` makes sure there are at least N played games**, playing whatever
is missing before the door opens, and prints their addresses. A floor rather
than a tally, because `./play` restarts the server on every change pushed to
the branch and hands it the same options each time: "play six" would play six
more on every restart. The analytics are the one part of this that cannot be
looked at without a finished game behind it, and playing one out by hand to see
whether a table renders is a poor way to spend an afternoon. Every seat is
played by the same heuristic the bots use, so these are real games rather than
walks through the rules, and the numbers on the page mean what they would mean.

**A game is stamped to the millisecond, not the second.** The order games were
played in decides what the ratings say, so a handful played back to back were
being put in whatever order their addresses happened to sort in and the ratings
followed.

---

## 13. The home page

Where a game comes from. Before it there was nowhere to stand: the root was
whatever board the server happened to be holding, the only way to deal another
was the lobby inside a game you were already in, and a game you had finished was
reachable only if you had kept the link. The wordmark carried a comment saying it
opened the lobby because there was no home to go to.

**Three cards, in the order somebody wants them.** New game, Tables, and what
you have played.

Like the report (§12), it is **a document rather than an application**: server
rendered, and with **no script at all**. Nothing on it changes without a request,
so there is nothing for a script to do, and the same rule that makes the report
honest makes this page work before anything has loaded.

- **New game** is one button, and it deals nothing: it leads to the lobby (§8),
  where the settings already live. It began as a form of its own with the four
  fields that decide what game it is, which was two forms asking overlapping
  halves of one question, and the half here was the smaller one. A page with no
  script can still start a game, because starting one is a link.
- **Tables** is somewhere to sit: every game in memory that has not been won.
  Your own are there whether or not you listed them, because you have to be able
  to get back to a game you dealt; other people's are there only if their host
  published them. A row says which turn it is on rather than how many turns are
  done, since that is what a live table has, and it is tagged **yours**, **listed**
  or both: the two answer different questions, and being listed is the one that
  cannot be taken back, so it is said out loud rather than implied.
- **Your games**, and only this visitor's. A game arrives in the history when it
  ends, and leaves the tables list at the same moment: one of the two lists has to
  give a game up or it appears twice, and the line between them is whether there is
  anything left to do at it. Each row offers the board and the report, one of them
  as the action and the other beside it, which one depending on whether the game
  finished.

  There was a second card under it listing every other game in the store. It was
  interesting while the store held six demo games and nothing else, and a browsable
  pile of other people's games on the front page as soon as it held anybody's.

**Nothing in the header.** Every other page's header offers a way back here and
a way to a new game. This page is the way back, and the card below is the way to a
new game, said properly and where the eye lands; a header link to the page's own
first button is furniture.

**Whose games are whose is a cookie, and the page says so.** A key handed to a
browser on its first request, sixteen characters, held for a year, `HttpOnly` and
`SameSite=Lax`. It is enough to answer "show me mine" on one machine and it is
not enough to answer "is this you" anywhere, so the card says it follows the
browser rather than the person and that an account is what fixes that. The key is
written into the game file as `by`, beside the moves rather than inside them, so
an account can claim a browser's games later without rewriting a single game.

The reader for that cookie **checks the value rather than trusting it**: our keys
are lower-case letters and digits and exactly sixteen of them, and anything else
is treated as no cookie at all. A key is only ever compared and stored, and that
is what keeps it so.

**The server holds sixteen tables, newest first.** It held exactly one for as
long as the only way to reach a board was to open the root, and a page listing
tables is a page whose links have to work. A table that falls off the end is not
lost, because every move writes its file; what it loses is the ability to be
played on, which is the right thing to lose first, and finished tables are
evicted before unfinished ones.

**A game outlives its table.** Restarting the server, or dealing sixteen more
tables, used to leave an unfinished game answering every click with "that game is
over": true of the table, false of the game, and there was no way back into it
short of abandoning something somebody was in the middle of. Asking to play a game
that is not on a table now puts it back on one, which is what writing a game down
as its moves was always for: seats, seed and the ordered steps rebuild the position
exactly, so a restart costs the table and not the game.

The clock comes back with it. Replaying the moves stamps each one at the moment it
is replayed, which would turn an hour-long game into a four millisecond one, so the
recorded times are put back and the session's own origin is wound back to the last
of them: what happens next lands after everything that already has. The time the
server spent stopped is not counted, because nobody was thinking during it.

**The table comes back too.** The file carries the lobby's answers as well as the
game: what the table is called, whether it is listed, the pace, the clock and its
increment, the discard allowance, whether the bank shows exact counts, and whether
a log is kept. They were not written down at all until the moment a game could be
taken up again, which is when it showed: the position came back exact and the
table came back with a different clock on it, which is the sort of thing you
notice second and cannot explain.

They live in a `Setup` beside the game rather than among it, because they are a
different kind of fact. Seats, seed and moves *are* the game and rebuild it
exactly; these are the arrangements around it, and a file that has lost them is
still a whole game played under arrangements nobody wrote down. Which is what a
version 3 file is, and it still reads: the settings default to what a fresh table
is, which is what those games were already coming back as.

Every setting is written out even when it matches today's default. A setting
absent because nobody chose it and one absent because it happens to match the
default read the same in the file, and stop reading the same the day a default
changes. The table's name is the one exception and is omitted when there is none,
the way the owner key is: an empty name is not a name.

**A game this build cannot replay is deleted.** If the rules have moved under it,
it is not a game any more, and leaving it is a row on the home page that refuses to
open, which is worse than either keeping it or losing it. This is the only thing
that deletes a game.

**A fresh server deals nothing.** It used to deal a table on start-up and on
every visit to the root, which put a game on disk for every time the page was
opened and closed again, each of them a seed and nothing else. Those were not
abandoned games, they were games that never started, and every figure the
analytics computed across the store was being divided by them. A table is dealt
when somebody asks for one, and its file is written by its first move.

**The history is capped at twenty-four rows** and says how many older games it is
not showing. A page read at a glance does not have a hundredth row, and nothing
is lost: every game keeps its address. "Across all of them" is a different page,
and it is specified in `analytics-backlog.md`.

**One logo, and it is the header's.** The page briefly carried two: a small
wordmark in a header above a large heading saying the same word. It carries the
application's header now (§14) and no `h1` of its own, because on this one page
the mark *is* the page's title: the mark is an `h1` here rather than a link, since
a link to the page you are standing on is an offer of nothing.

The page borrows the report's stylesheet whole, so the tokens, the header, the
card, the table and the tooltip are one design rather than two that resemble each
other. What it adds is a button, which the report has none of.

---

## 14. The header

Four screens wear it: the board, the lobby, the report and the home page. It was
three headers and a gap before this. The board had a mark, a table name and two
links; the report had a mark and one bare `nav` link in the body face; the lobby
covered it with a full-screen sheet and put a second wordmark inside that sheet;
the home page had none. Nothing about that was visibly broken, which is the
problem with it: an application whose header changes between pages reads as
several applications that share a colour.

**The mark, what you are looking at, and the ways out of it**, in that order, with
the links pushed to the far end so the mark and the name stay together as one
thing rather than reading as the start of a list.

- **The mark** is Audiowide at 22px in the accent, and links home from everywhere
  it is a link. On the home page it is an `h1` and not a link: that is the one page
  whose title is the name of the thing, and a link to the page you are standing on
  is an offer of nothing while a page needs a heading more than it needs that.
- **The context** is a `.gameName` beside the mark, in the body face and a quieter
  colour, so the mark stays the mark. The board uses it for the table's name. The
  report and the home page have a heading of their own beneath and leave it empty.
- **The links** are `.headLink`, all of them anchors. **New game** means `/lobby`
  on every page that offers it, which it did not: on the board it used to be a
  button that opened the lobby as a sheet over the game, so one label had two
  behaviours depending on where it was pressed.

**It belongs to the window, not to the column.** There is one gutter for the
whole application, `clamp(16px, 2.2vw, 32px)`, declared as a token in both
stylesheets, and the header is inset by it on every page: the mark lands on the
same pixel from the same corner whichever screen you are on, and so do the links
from the other one. It was briefly centred with the report's column instead,
which lined it up with the heading beneath and put it somewhere different from
where the board keeps it. Being in the same place on every page is worth more
than being level with one thing on two of them, which is where a site's mark
lives anyway.

The same gutter also sets the board's rails, so on the board the mark and the
column under it do line up. The board is tall rather than wide on any large
screen, so the width this costs is width that was sitting empty beside it.

**The same ground under all four.** The board was a warm grained table with three
faint pools of colour washed across the top of it; the report and the home page
were flat cream; the lobby painted a fourth wash of its own over whatever it
covered. All four are the table now, the same three pools and the same grain at
the same scale and weight. This was most of what made the screens look like
different applications: the header was the part you could name, and the paper was
the part you could not.

**The board's copy is written by hand**, in `assets/index.html`, because that page
is one file with its own stylesheet and its own tokens. The classes and the rules
are named the same on both sides, `mark` included, which is what the rename was
for: one thing with two names is two things waiting to be edited apart. Two tests
read the board page's own markup alongside the two rendered ones and fail if they
stop agreeing about the header's shape, the gutter, the pools or the grain.

The drift those tests are for is small and invisible one page at a time. The mark
sat three pixels lower on the rendered pages than on the board for no better
reason than that one of them set a line height and the other left it to the
font's metrics.

Both server-rendered pages carry the tab icon the board always had. They had none,
so every visit to the home page or a report asked for `/favicon.ico`, got a 404,
and left it in the console.

---

## 15. The dock never wraps

The strip of controls under the board is the one thing in this layout with a hard
minimum: it is a row of cards and buttons at a size you can read, and below that
width it has nowhere to go but a second row. It kept finding one.

**The rule.** Shrink as far as it takes to stay on one row and no further, and
wrap only when shrinking more would cost more than a second row does.

Three things make that true, and all three are easy to undo by accident.

- **It is sized by the column it is in.** It used to be `clamp(11px, .74vw, 14px)`,
  on the reasoning that the dock and the space it has to fit both grow with the
  window, so whether it fits is very nearly constant. Very nearly: the rails either
  side stop growing at 400px and the window does not, so the middle column grows in
  jumps the dock's type did not follow. A container query is the honest version of
  the same reasoning, `1.16cqw`, one per cent of the column the dock is actually
  in. An element cannot query its own container, so the dock sits in a cell that is
  one.
- **Every measurement inside it is in `em`.** That is what makes the strip one
  shape at one scale, so its width is a fixed multiple of its type size and the
  coefficient can be the reciprocal of that multiple. It is measured, not tuned:
  85.4 type sizes wide with both award tiles in hand, which is the widest the strip
  is ever asked to be. A `px` gap anywhere in it breaks the arithmetic.
- **The push between your hand and the controls costs nothing.** It was an empty
  group with `flex: 1`, which did the job and spent two of the dock's gaps on a
  thing with no width. An auto margin is the same gesture and takes space that is
  spare.

The rails gave up some width too, from 20 and 21 per cent to 17.5 and 18.5. They
keep their floors, so the narrow windows where they were sized up are unchanged;
what changed is the band from a laptop to a large monitor, where they were holding
width the dock needed and the seat rows did not.

Measured across ten window widths, with no award tile, with one and with both: one
row everywhere from 1366px up. Below that the type is on its floor of 9px, where
the uppercase labels are just over seven and are the last size at which they are
still words, and it wraps. The board does not fit those windows anyway (§11).

The awards group is the reason this kept surfacing: it appears only when you hold
a tile, so the dock wrapped at the moment you took the longest road.

---

## 16. The second seat

Two people at one table. The engine was always able to run a four-player game;
what could not was everything around it, and all of it for the same reason: one
sentence, written in about forty places, saying *the human is seat nought*.

**A person is a property of a seat, not a constant.** `Session` carries which
seats have people in them, and every question that used to be asked about seat
nought is asked about a seat: what you can see, what you may choose, whose turn
it is, whose offers are yours, which card is waiting on you. The seat-nought
versions are still there as one-line wrappers, because a table with nobody else
at it is still the common case and should read like one.

**What the difference between a person and a bot actually is**, once it is
written down: a person's seat waits to be asked and a bot's answers immediately.
That is the whole of it, and it turns up in three places.

- **The bots stop for anybody.** The loop used to break on "it is seat nought's
  turn, or something is being asked of seat nought". It breaks on any person's
  turn and any person's pending question, which is the same sentence with the
  seat quantified.
- **The market is settled for bots only.** A person's card sits on their screen
  and the table waits for it. Before, the settle ran from seat one upwards, which
  is the same thing as long as the only person is at nought.
- **The clock forces a person's turn**, whoever holds it, and clears every
  person's unanswered offers rather than one seat's.

**Every seat gets its own view.** The page is rendered per seat: the hand, the
development cards, whose turn it is, which offers are yours, and the numbered
list of choices a click comes back as. Two people are served two of these and
neither is ever sent the other's cards, so there is nothing to hide in the page
because nothing private ever arrives in it.

An action names an index into *that seat's* list, and the server applies it as
the seat the asking key is sitting in. So one person cannot press another's
button: the only thing they can name is something on their own screen, and a
seat with nothing being asked of it has no list at all.

### Joining

**A table with an open chair has not started.** That is the whole reason the
state exists. Joining a game already in progress means being handed whatever a
bot built for you over forty turns, which is not joining a game; letting people
in only before the first move is the difference between a table filling up and a
table being walked into.

So the first move is the door closing, and nothing can move until the seats are
settled. A table waiting for people shows the board it was dealt, says how many
it is short, and offers its link. The host, and only the host, can give up on the
empty chairs and hand them to the house bot: everybody else at the table is
waiting for the same person they are, and one of them deciding for all of them
would be a different rule.

**Sitting down is deliberate, and carries a name.** Opening the table is not
joining it. A reader with no seat at a table still filling up is asked once
whether they want one, because their one chance closes at the first move and a
page that quietly let them watch would have spent it for them. What it asks is
the only thing the table does not already know: what to call them.

### Leaving

**Standing up means two different things, and which one depends on the first
move.** Before it, nothing has happened: the chair goes back to the table for
somebody else to take, the name goes with it, and the table is short again.
After it, the seat is part of a game in progress and cannot be handed back,
because the other players are owed an opponent rather than a gap. It stays
theirs, the house bot plays it, and they can come back to it. The control says
which of the two it will do.

**Going away without saying so is the same thing.** The page asks for the state
every three seconds, so a seat that has not been heard from for two minutes has
had its tab closed rather than its owner thinking. That seat is played by the
house bot until its person's page asks again, at which point it is theirs once
more: coming back is the ordinary path and not a second kind of joining.

Presence is not stored anywhere and is never written down. A seat is present
because somebody just asked about it, which is as direct a measure as this server
can take, and a restarted server has correctly heard from nobody.

**Except when nobody is there at all.** One person gone is a seat the bots cover
so the game carries on for everybody still at it. Everybody gone is a game that
must not finish without them, so the table waits for all of them. A game that
played itself to the end while the room was empty would be a game destroyed
rather than a game continued, and that is the whole reason this is a rule about
the table rather than a filter over the seats.

**The host standing up before the start hands the table on.** Whoever is sitting
in seat nought may fill the empty chairs and begin, as well as whoever dealt it.
Without that second half a host who changed their mind would leave their table
waiting on somebody who had already gone, with nobody able to start it.

**A table waiting for nobody is closed.** Twenty minutes after anybody last
looked at it, an unstarted table holding an open chair stops existing. That is
measured from the last request about it rather than from when it was dealt,
because the page polls every three seconds: a host still at the screen holds
their table for as long as they are there, and a host who closed the tab holds
it for twenty minutes. The waiting room says so out loud, since it is the one
thing on that screen that happens without anybody pressing anything.

Only tables that never started, and there is nothing to write down because
nothing happened at them: the store never had them. A game with moves in it is
somebody's afternoon and is never swept, whatever it is short.

Which turned up a rule that had been quietly broken. **A table still filling up
is not written to disk.** Sitting down and filling the chairs both wrote the file
straight away, which put every dealt-and-abandoned table into the store for the
analytics to divide by. That is the same rule that has always kept a game nobody
moved in out of it; the first move writes the file, with everybody's seat and
name in it.

A reader whose table is closed while their page is open is told on the page
rather than bounced to the home page: a tab that silently became a different
screen is worse than one that says what happened to the last one.

**Coming back is by key, and always allowed.** A seat you were in is yours
whatever has happened since, including the server restarting. The rule is that
you cannot take a *new* seat in a game under way; nothing stops you returning to
your own.

**A full table is somewhere to watch.** Somebody with no seat gets the
spectator's fog, which is the public position and nobody's hand (P-6). It is
rendered for a seat no table has, which is what makes it safe rather than
careful: every private field is keyed off that seat, so the hand it is not
holding is a hand of nothing. The page marks itself as watching and puts the
controls away, because they would be controls for a seat the reader does not
have.

**The seating is written down** (format 6, a `chair` line per seat): a person's
key and their name, or `bot`, or `open`, in seat order. One line each because a
name is somebody else's text and has spaces and commas in it, so everything after
the first word is the name and there is nothing to escape. So a restart puts everybody back in the chair they were
in, and a game you were invited to is on *your* home page as well as the host's,
because the chairs say you played it and the dealer's key alone never could.

**On the home page** a table with a chair free says so, in the one tag on that
page that is not quiet: it is the only thing there you can be too late for. The
action reads **Sit down** for a chair going, **Back to it** for a table you are
already at, and a quiet **Watch** for a full one.

**Every seat has a name.** It used to be one name, the host's, because there was
one person. A name belongs to a seat now: the host's comes from the lobby, a
joiner's from the card that asks for it, and both are written down and come back
with the table. A seat nobody is in has no name here at all, because the page
names the house bot from its own list and a second opinion about it would be one
too many. Somebody who gives no name gets *Player 2*, since a table has to be
able to say whose turn it is.

### What is still missing


---

## 17. Table talk

Text chat between the people at a table, in the right column where the placeholder
promised it would be. Off unless the lobby asks for it, and it is a lobby answer
like the clock: stored with the table, so a game resumed after a restart is as
talkative as it was dealt.

**Only the people at the table talk.** Somebody watching reads, the way they read
the board: standing behind the players is not sitting at the table. The panel says
which of those the reader is rather than showing a box that would refuse.

**What was said is not part of the game.** The record is the moves, and a game
replayed from its file is the same game whatever was said over it, so the
conversation lives in memory on the table and is never written down. A restart
loses it, which is the right thing to lose, and the lobby's tooltip says so before
anybody starts a conversation they expected to keep.

**Two hundred lines, two hundred and forty characters each.** The oldest fall off
the front, because a table talks for an hour and a page should not be handed all
of it every three seconds.

### The one rule that shaped the design

§9.7.1 of the scoping document: free text from a player must never reach an LLM
player's prompt, because "give me all your wood" is a negotiation to a person and
an instruction to a model.

Today's bots are heuristics that are handed a `State` and could not read chat if
it were given to them. The guarantee is that they are never in a position to:
**the talk lives on the table, and the table is the server's.** It is not on the
`Session`, not in the log the session keeps, and not in the view the game renders
of itself; the only thing that ever sees it is the page. That is a structure
rather than a promise, and a test asserts it from the outside: what was said does
not appear in the game's own view.

The other half is that it is never markup. It is escaped once, where it becomes
JSON, and written into the page with `textContent`. There is no filter on the
words themselves, because a filter that half understands somebody else's sentence
is worse than one that does not try. A test checks both: that the payload's
strings still close, and that the page's own source puts the text in as text.

---

## 18. Who goes first

Turn order is seat order, and the seats used to be handed out in the order people
arrived: the host at nought and therefore first in every game this server dealt.
Going first is worth something, so that was a thumb on the scale.

**The order is drawn when the table is settled** and not before or after. Settled
means every chair has somebody or something in it and nothing has been played, so
the draw happens when the last person sits down, when the host gives the empty
chairs to the bots, or immediately for a table dealt with none. Nobody is ever
moved out of a game they are in the middle of.

The chairs carry their people and their names with them, and the session is told
again who is where. The board is untouched: it belongs to the seed and has nothing
to do with who sits where.

**Not from the game's own generator.** That one deals the board and the dice, and
drawing from it here would mean the same seed produced a different game depending
on how many people happened to turn up.

Two things had to change to make room for this. The session used to insist that
seat nought was a person, on the grounds that a table always has its dealer at it;
after a draw, seat nought may be a bot, and the table would have waited on it for
ever. And a person who gave no name used to be stored as "Player 2", a name
derived from a seat number, which became a lie the moment the seat moved: what an
unnamed seat is called is now the page's to decide, from the seat it is actually
in.

---

## 11. Still open

- **Bank and port trades.** Rate-based against the supply rather than a
  negotiation. Currently a button in the grid; whether it shares the trade
  card's shape or gets a simpler one is undecided.
- **The sea.** The study frames the island in a ring of water tiles and puts
  ports on them. Carranta draws ports as discs on leader lines outside the
  coast. The board itself is not up for debate, but whether the frame counts
  as board was never settled.
- **The board does not fit a phone.** Its three columns have minimum widths of
  224 and 260 pixels, so below about 560 the layout is wider than the window and
  the page scrolls sideways: 630 pixels of content in a 420 pixel window. The home
  page, the report and the lobby all reflow to that width; only the board does
  not, and it needs a layout for one column rather than a smaller gutter. Long
  standing, and measured here rather than assumed: the shared gutter costs eight
  pixels of it and did not cause it.
- **Talking before you sit down.** Only people in seats may say anything: the
  panel tells a watcher they are watching rather than showing them a box that
  would refuse. That follows the same line everything else here follows, that
  standing behind the players is not sitting at the table, and it is the line
  worth reconsidering first. Somebody deciding whether to take the last chair
  cannot ask about the game they are about to join, and "is anybody actually
  playing this" is a fair question to want to ask from outside. The cost of
  opening it is that a table's conversation becomes something anybody who can
  reach the server can join, which is a moderation question (P-16) rather than a
  UI one.
- **A conversation that outlives its table.** Chat is in memory on the table, so
  it exists only while the table does: it is not on the home page beside a listed
  game, and asking "is this one any good" before joining has nowhere to happen.
  Making that work means chat outliving the table it belongs to, which means
  writing it down, which is the decision §17 deliberately did not take: the record
  is the moves, and a transcript is the one thing in this system that carries
  personal content rather than game content. The scoping document already settled
  what that would cost, H-8: logs indefinite, chat ninety days, precisely so a
  deletion request stays satisfiable without touching immutable game history.
  Worth doing, and not worth doing by accident.
- **Accounts.** The home page answers "show me my games" from a key in a cookie,
  which follows a browser rather than a person. The key is written into every game
  file, so an account can claim one browser's games later without touching a
  single recorded game; nothing claims anything today.
- **Bot names.** Invented and placeholder until told otherwise.
- **The endgame.** Neither the study nor these decisions say what winning
  looks like.
