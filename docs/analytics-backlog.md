# Analytics backlog

What the analytics do not answer yet, and why each one is worth answering. Two
kinds of thing live here:

- **Across games.** A report on one game is the wrong place to make a claim about
  many, so anything that needs a corpus waits for the page that reads the whole
  store. Most of this file is that page's brief.
- **Per game, not yet built.** The gaps two reviews of the per-game page turned up.
  All twelve are closed now, and that section says what is left instead: the two
  kinds of question one recorded game cannot answer.

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
    target's score at the time, pooled. Needs the per-turn score the report already
    samples, which is not yet stored across games. The per-game blockade table
    already shows the demo's winner was blockaded most, which is one game and
    therefore not evidence.
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

The twelve gaps two reviews turned up are closed. The per-game report now splits
spending four ways, measures what each road did, counts the three walls a seat can
be stuck behind, carries the ask that was in every offer and what it was asking
for, times every development card from draw to play, breaks discards down by card,
measures the endgame as a race, reports coverage per resource, and says what kind
of board was dealt.

What is left is not gaps in the per-game page. It is the corpus page above, and two
things the per-game page cannot answer by construction:

- **Anything that needs a counterfactual.** "Could the table have stopped them"
  wants a game that was not played. The race table gives the turns available, which
  is as close as one recorded game gets, and the honest answer to the rest is a
  search rather than a report.
- **Anything that needs a second opinion.** Whether an opening was *good* is a
  question about every board and every game, not about this one. That is the corpus
  page, and the fifteen above are its brief.

A per-game figure worth adding will most likely come out of the corpus work rather
than out of another read of the page: the moment there is a distribution to sit a
game against, half the columns here gain a percentile they cannot have today.
