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
│  header, the mark linking home · this game's name           │
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

**The board never moves.** Its frame is measured from the land and the ports
alone, which do not change for the life of a game. Measuring everything drawn
meant that a piece placed past the previous extent shifted and rescaled the
whole board, which is the one thing a board must never do. The only thing that
resizes it is the window.

**The header is the mark and the table's name, and nothing else.** Whose turn
it is, the turn number and the seed all belong to panels that already say
them, and repeating them at the top made the one strip that never changes the
busiest thing on screen. The mark links home; the name sits beside it in the
body face and a quieter colour, so the mark stays the mark and the name reads
as the answer to "which game is this". An unnamed table shows nothing rather
than a placeholder, since a name nobody chose is worse than no name.

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

Development cards sit in their own group beside the resources. **You play one
by clicking it**, so there is no separate button opening a list of the cards
you are already looking at. A card bought this turn is marked `new` and cannot
be played until the next one (R-9.4): the engine refuses it either way, but a
refusal you cannot see in your own hand reads as the interface being broken.

**Before the roll, only the militia is playable** (R-9.5), and the other piles
say so rather than simply declining to click. It is the one card whose timing
changes anything, since moving the robber before production decides which
hexes pay this turn.

**A victory point card is the exception, and is not a card you play.** It
scores the moment it is bought, it is yours alone until the game ends, and no
action exists behind it (R-9.11). It carried the `new` badge and offered
itself as playable next turn, which described a card that does not exist. Its
pile is inert now and says what it is instead.

### Actions

A fixed grid: **roll, buy a development card, trade, end turn.**

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

It is decided on the **public** count for every seat, including yours, because a
standing has to be one measurement. Marking it off whatever each row happens to
display would put you in front on a basis nobody else at the table shares, and a
victory-point card in your hand is exactly the thing that is not in front
publicly. When your real total would lead and your public one does not, your own
tooltip says so, which is the honest version of the same information.

**Nobody is marked while everybody is level**, which is the whole table for the
first few turns. Four accented numbers would announce four leaders about a game
where the word does not apply yet.

---

## 5. Where a decision happens

The rule: **anything with a place on the board happens on the board; anything
without one happens in a card that waits for you.**

| Moment | Where |
|---|---|
| Build a road, settlement, city | Board, click the ghost |
| Setup placement | Board, click the ghost |
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

**Composing:** click a stack in your hand to move one card into *you give*;
click it in the tray to take it back. Wanting is the same gesture against the
five resources. The offer is built out of the cards you are already looking
at, rather than in a disconnected panel of steppers.

**One door for every trade.** The bank had its own button in the dock opening
its own list of twenty sentences, which split trading in two and made the trade
people make most often the one they could not simply say. **Trade** now opens
the composer whether the other side is a person, a port or the bank.

A bank or port trade is one resource in and one out, which is exactly the shape
the composer already builds. Whenever what you have built is a trade the bank
or a port will take, **the rate appears as a button under the composer**. When
it is not, the card says what the bank would take instead, so the option is
never invisible until you stumble onto the right count. A port's improved rate
is named as a port rather than shown as an unexplained smaller number.

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
cannot cover says "you cannot cover this" and can still be turned down. Saying
why it cannot be covered is safe here and only here: it is your own hand.

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

**A proposal of yours stays carded until the turn ends.** Before this it went on
the table and then said nothing: you got a result or you got silence, with no
way to tell which was still coming. The requirement is that it survives at least
until everyone has answered; ending it with the turn is what the engine does
with the offers themselves, so the card and the market disappear together.

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

**Four sounds, and each one names an event rather than an interaction.**

| Sound | Plays when |
|---|---|
| `dice-throw-3` | The dice are rolled, by anyone |
| `impact-generic-light-002` | A piece goes down on the board |
| `card-place-1` | Cards are dealt into a hand |
| `drop-002` | An offer goes on the table |

**They are played off the log, not off the clicks.** The log is the record of
everything that happened, including everything three bots did while nobody was
looking, so hanging the cues on it makes a move sound the same whoever made it.
Hanging them on the click would have made the whole table silent except for you,
which is the opposite of the point: the sounds are here so a player can tell
what is going on without watching every corner of the board.

So a "dealt card" is any line that puts cards into a hand: production, a card
bought, a card stolen, a trade landing. A "placed piece" is any line that puts
wood on the board: a settlement, a city, a road, and the robber, which is the
militia's whole effect and a seven's. Within each group they are one event to
the ear, and one sound saying "a piece went down" is worth more than four saying
which piece.

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
