# Analytics backlog

What the analytics do not answer yet, and why each one is worth answering. Two
kinds of thing live here:

- **Across games.** A report on one game is the wrong place to make a claim about
  many, so anything that needs a corpus waits for the page that reads the whole
  store. Most of this file is that page's brief.
- **Per game, not yet built.** The gaps a review of the per-game page turned up
  that have not been closed. Six were, and the section says which.

Nothing here is a promise about order. The rule from §10.1 governs all of it:
**small n makes p-values invalid, large n makes them uninformative**, so
everything below is specified as an effect size, a percentile or a rank, and
never as a significance claim.

## Across games: the corpus page

This page does not exist yet. Every figure named here needs a corpus, and the
per-game report deliberately refuses to make any of these claims from one game.

### Moved off the per-game page

- **Seat win rates.** Was going first worth anything? It was on the per-game
  report and should not have been: four seats and one game is one bit of
  evidence about a question that needs hundreds. The corpus already computes
  `seat_win_rate`; it needs somewhere honest to appear, with a confidence
  interval and the game count beside it.

### The fifteen from the data-driven-strategy piece

Taken from a public write-up on analytics for this genre
(`duddhawork.com/blog/catan-analytics-how-to-win-with-data-driven-strategies`),
cross-referenced against §10.3 so that what the scoping document already
promised is not duplicated here. Every one is a corpus question.

1. **Win rate by opening pip count.** Bucket openings by total pips, plot win
   rate per bucket. The obvious first question and the easiest to get wrong: pips
   correlate with turn order, so it has to be conditioned on seat.
2. **Win rate by opening coverage.** The same for the coverage figure the opening
   card already computes. The interesting version is pips *and* coverage
   together, since the whole point of coverage is that it separates openings the
   pip count calls identical.
3. **Win rate by resource mix in the opening.** Which of the five a placement can
   and cannot produce, as a presence pattern rather than as amounts. Ore-and-wheat
   against wood-and-brick is the oldest argument in the genre and it is settleable
   here.
4. **Ports: worth building on or not.** Win rate for seats that opened on a port
   against seats that did not, split by the port's rate and by whether the seat
   could produce what it discounted. The per-game board card already measures how
   good this board's port spots were, which is the covariate to condition on.
5. **First settlement position value.** Win rate by the *intersection* the first
   placement took, aggregated over boards by pip count and adjacency class rather
   than by absolute position, since no two boards share coordinates.
6. **Development cards against buildings.** Win rate by the share of a seat's
   spending that went on cards, which the per-game building card now splits out
   four ways.
7. **When to buy the first city.** Win rate by the turn a seat's first city went
   up, normalised by game length. The timeline strip records the turn already.
8. **Longest road: worth chasing?** Win rate for seats that held it at the end
   against seats that contested and lost it. The timeline records tiles changing
   hands, so "contested" is available rather than inferred.
9. **Largest militia: the same question**, plus the interaction with card
   buying, since militias arrive as cards.
10. **Trade volume against winning.** Win rate by trades completed, and
    separately by cards moved, which the per-game trades card now measures. The
    likely finding is a confound: winners have more to trade.
11. **Who feeds the winner.** Across games, does trading with the eventual winner
    correlate with losing? The per-game column exists; the claim needs a corpus.
12. **Supply-trade dependence.** Win rate by share of trades made against the
    bank or a port rather than against a person, which is a proxy for a table
    that would not deal with you.
13. **Robber targeting.** Is the leader targeted? Robber placements against the
    target's score at the time, pooled. Needs the per-turn score the report
    already samples, which is not yet stored across games.
14. **Discard exposure and outcome.** Win rate by turns spent over the discard
    limit, which the per-game ledger now counts.
15. **Turn order and length.** Does the seat-order advantage change with game
    length, and does it change with player count?

### Generator fairness, which is a different question

Also from §10.1, and pointedly not a per-game question: with millions of pooled
rolls, chi-squared becomes valid and useless at the same time, so the corpus page
judges the generator on effect size and on **independence**, since a bad
generator can produce correct marginals with serial structure. Runs, lag-1
autocorrelation and a pair-frequency table, all as effect sizes, and any p-values
behind Benjamini-Hochberg (`stats::benjamini_hochberg` exists for exactly this).

### Ratings, once there are enough games

- Calibration: do rated favourites win at the rate their ratings imply?
- Rating volatility per player, and how many games it takes to settle.
- Whether the three bot seats converge on each other, which they should, since
  they are the same player. If they do not, the rating model or the seat
  advantage is wrong, and the two are distinguishable.

## What the analytics have already found

Not a backlog item, but the reason the backlog is worth working: two findings that
came out of building the cards, both of which are about the *engine* rather than
about the analytics.

- **The bots never trade with each other, and the offers table says why.** Across
  a hundred and fifty-four offers in one demo game, not one was accepted. Every
  seat asks about 2.4 cards for 1.3 put up, an ask of 1.86 to one, and no seat will
  take that from another seat pricing the same way. Every trade in these games is
  with the bank or a port. That is a heuristic-tuning ticket, and it also means
  every corpus figure about player-to-player trading is measuring an empty set
  until it is fixed.
- **A quarter of some seats' turns end unable to spend.** The building card's stuck
  column runs to a fifth of the game for the winner in the demo: cards in hand, the
  price of a settlement covered, and nowhere legal to put one. Whether that is the
  board, the road network or the bot not planning towards spots is worth knowing,
  and the figure exists now to tell.

## Per game: still missing

Six gaps from the first review have been closed: the built column is split by what
it bought, the offers carry their ask, roads have a length, blocked builds are
counted, the endgame is measured as a race, and coverage is reported per resource.
What is left:

- **What a road was built *towards*.** The building card gives a road network's
  length. It does not say whether a road opened a new settlement spot, chased the
  tile, or went nowhere, which is the difference between a plan and a habit. Needs
  the spots reachable before and after each road, which `settlement_spots` can
  answer at every step.
- **Blocked cities and roads.** Stuck counts settlements only. A seat holding a
  city's price with no settlement of its own to upgrade is in the same position,
  and so is a seat out of road pieces.
- **Offers by counterparty.** The ask is a per-seat average. Who each seat aimed
  its offers at, and whether the ask changed with the target, is the difference
  between a bad price and a grudge.
- **Development cards in time.** The timeline marks a card bought. When each kind
  was *played*, and how long it sat in hand first, is recorded in the moves and
  unshown: a militia held for forty turns is a different decision from one played
  on draw.
- **Discard composition.** The ledger counts cards discarded and the exposure
  behind them. Which cards a seat chose to throw away is a decision, and it is in
  the moves.
- **The board's own difficulty.** The board card compares this deal to an average
  one for pips. Whether the *numbers* clustered, whether the resources clumped, and
  whether any intersection was worth more than any other on this deal, is a
  board-quality question the card only half answers.
