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
table is listed, an invite link, the turn clock, whether the table keeps a log,
and a seed. It stays configurable **while people join**, which is what makes it
a lobby rather than a settings dialog.

**A table is private unless it asks to be listed.** Listing is publishing, and
publishing is the answer that cannot be taken back, so it is the one that has
to be chosen rather than the one that happens by default. The server agrees
independently: anything other than an explicit `public` leaves the table
unlisted, so a missing or misspelled setting cannot publish a game by accident.

Public is **shown but not selectable**, because there is no landing page for a
listed table to appear on. Shown rather than removed: knowing the choice is
coming is worth more than a row with nothing to compare, and the reason it is
unavailable is on the option itself. It is `aria-disabled` and keeps its tab
stop, so that reason stays reachable by keyboard and by touch.

Visibility is stored on the session rather than in the browser that dealt it,
since a listing will be read from the table and not from whoever is looking at
it. Nothing lists tables yet, so nothing reads it yet either.

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

The subtitle under a heading survives on exactly two cards, the dice and the
robber, because on those it carries a figure no table holds. A sentence
describing a card the reader is already looking at is a sentence they read to
learn nothing. The only paragraphs left anywhere are answers rather than
explanations, like "Nobody finished a turn in this game".

**A seat wears its colour, immediately left of its name, in every table.** The
same mark the board plays in, in the same place every time, so a row can be
found by colour rather than by reading down the names. Row labels and the
robber's column headers alike.

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

- **The result**, broken into what scored. The five things that do (R-11.3),
  each as how many were held with what they were worth in brackets, so the
  bracketed figures add across to the total: a settlement one, a city two, each
  tile two, each victory point card one, and a road nothing however many there
  are. Counted off the final position rather than off what was built, which is
  a different number, since a settlement upgraded to a city stopped being a
  settlement and was still built. The total is the true one, hidden cards
  included, which is not what the table could see while it was playing.
- **The turns**, per seat: turns, moves, longest turn, time and share of the
  game's moves. A turn is what falls between two ends of turn, and it counts
  everything that landed inside it, the turn holder's or not, since a discard, a
  robbery and an accepted offer all happen in somebody's turn; the time column
  is wall-clock on the same basis. Setup placements are left out: they come
  before anybody has a turn to take. The time column appears only when the file
  has a clock in it, so a game saved before version 2 shows the rest and says
  nothing about time.
- **What it did to the ratings**, which is the section this page exists for.
  Before, after, and the change, per seat. A Weng-Lin Plackett-Luce update over
  the whole finishing order rather than just the winner (A-1). The figure shown
  is the conservative estimate, three standard deviations below the mean, and
  beside it the games each player had behind them, because that is how much the
  number is worth believing. Ratings are computed by replaying every recorded
  game in order and reading the pool either side of this one: a rating is a
  function of everything before it, so what a result did cannot be worked out
  from the result alone.
- **The dice**, as a roll histogram against the theoretical one and a KL
  divergence in bits, then placed against every other game recorded here as a
  percentile. **No p-value**, deliberately (§10.1): across enough games one in
  twenty clears p<0.05 by construction, and those are precisely the games
  somebody screenshots as proof of rigging. Until there is a second game the
  percentile is withheld and says so, because a percentile of one game is not a
  percentile.
- **What the board paid**, decomposed (§10.2): expected production, what the
  robber cost, what the supply denied, and the dice term, which is the only
  genuinely random one. The dice term is also given in standard deviations,
  which is the figure to read.
- **The robber, the market, development cards and the opening**, as counts.
- **Across every game here**, seat win rates, with the note that at a handful of
  games the spread is noise.

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
