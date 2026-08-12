# CATAN — Rules & Materials Scoping Document

**Source (primary):** *CATAN — The Game* rulebook, CN3081, v6.250401 (6th Edition), © 2025 CATAN GmbH / CATAN Studio. 12 pages.
**Source (secondary, for edge-case rulings only):** official CATAN base-game FAQ and the 5th Edition *Rules & Almanac* — see [§14](#14-sources).
**Status:** Draft 7 — rules, engine architecture, history/data model, platform scope, bot strategy, and analytics/rating design recorded. 29 decisions registered.
**Target:** **Digital implementation** of the base game: a Rust engine core (§6) usable both as a game service and as a high-throughput environment for AI training, with full game history capture (§7), a lobby-based multiplayer platform (§8), heuristic and LLM opponents (§9), and a statistics and rating layer (§10). This document is the reference for the state schema, action model, rules validation, data model, product scope, and analytics methods.

**Scope boundary**

- **In scope:** base game, **3–4 players**, both setup variants (Fixed and Variable).
- **Out of scope:** the 5–6 player extension (and its Special Building Phase), Seafarers, Cities & Knights, Traders & Barbarians, and all scenarios/promos. The state model should not hard-code a player count of 4, but no expansion mechanics are specified here.

**Conventions used here**

- Rules are numbered `R-x.y` so they can be referenced from tickets, tests, and code.
- Every rule is traceable to a source: `p.N` = page of the CN3081 rulebook; `FAQ` / `ALM` = official FAQ / 5th Edition Almanac (used only for clarifications, marked as such in [§12](#12-resolved-rulings-edge-cases)); `HOUSE` = a project decision with no source in any official material; *inferred* = read from the rulebook by implication rather than stated.
- Facts read from **diagrams/artwork** rather than rules text are marked *(image-derived)*.

---

## 1. Game parameters

| Parameter | Value | Source |
|---|---|---|
| Players | 3–4 (in a 3-player game the white pieces are not used) | p.5 |
| Win condition | First player to reach 10 VPs **on their own turn** | p.2, p.10 |
| Turn order | Clockwise, starting with the first player | p.6 |
| Turn structure | Production phase → Action phase | p.6 |
| Setup variants | Fixed Setup (recommended for first game) / Variable Setup | p.4, p.11 |
| Board topology | 19 hexes, 54 intersections, 72 edges | derived |
| Designer | Klaus Teuber (1952–2023); ongoing design Benjamin Teuber | p.12 |

---

## 2. Rules

### 2.1 Objective (R-1)

| ID | Rule | Source |
|---|---|---|
| R-1.1 | The first player to reach 10 victory points (VPs) on their turn wins. | p.2 |
| R-1.2 | VPs are earned by building. Resources needed for building are collected and traded for. | p.2 |
| R-1.3 | VP sources: settlement = 1 VP, city = 2 VPs, Longest Route tile = 2 VPs, Largest Army tile = 2 VPs, each Victory Point development card = 1 VP, road = 0 VP. | p.8, p.9, p.3 |

### 2.2 Setup — Fixed Setup (R-2)

| ID | Rule | Source |
|---|---|---|
| R-2.1 | **Assemble the frame.** Match the numbers at the puzzle-piece ends of the 6 sea frame pieces to assemble the coast of Catan. | p.4 |
| R-2.2 | **Place hexes and number discs** inside the frame exactly as shown in the setup diagram (fixed, prescribed layout). | p.4 |
| R-2.3 | **Create the supply.** Sort resource cards by type into five **faceup** stacks in the card trays. Shuffle the development cards into one **facedown** stack in the remaining card tray slot. Place the Longest Route and Largest Army tiles near the board. | p.4 |
| R-2.4 | **Place the robber** on the desert hex. | p.5 |
| R-2.5 | Each player selects a color and takes that color's roads and buildings (settlements + cities) plus a player aid. In a 3-player game the white pieces are not used. | p.5 |
| R-2.6 | Place 2 starting settlements and 2 roads per player, at the prescribed positions shown in the diagram. | p.5 |
| R-2.7 | **Starting resources.** Each player takes from the supply the resource cards matching the hexes adjacent to their **second** settlement (highlighted in the diagram). These cards are kept **hidden in hand**. | p.5 |
| R-2.8 | **First player.** Each player rolls the dice; highest roll is the first player. | p.5 |

### 2.3 Setup — Variable Setup (R-3)

| ID | Rule | Source |
|---|---|---|
| R-3.1 | **Assemble the frame.** Shuffle the sea frame pieces and connect their puzzle-piece ends (random coast, port positions vary). | p.11 |
| R-3.2 | **Place the hexes** randomly, face up, inside the frame. | p.11 |
| R-3.3 | **Place the number discs.** Arrange the discs facedown in A-B-C (alphabetical) order. Starting at any corner of the board, place them on the hexes counterclockwise, **skipping the desert**. Then flip them so the number side is faceup. | p.11 |
| R-3.4 | **Place the robber** on the desert hex. | p.11 |
| R-3.5 | **Create the supply** — identical to R-2.3. | p.11 |
| R-3.6 | **First player.** Each player rolls the dice; highest roll is first player. Then each player selects a color and takes their roads, buildings, and a player aid. | p.12 |
| R-3.7 | **Placement round 1.** The first player places 1 settlement on an empty intersection of their choice, then 1 road on an empty edge adjacent to that settlement. Continue **clockwise** (to the left) until every player has 1 settlement and 1 road. | p.12 |
| R-3.8 | **Placement round 2.** Starting with the last player and going in **reverse order**, each player places 1 settlement on an empty intersection of their choice and their second road on an empty adjacent edge. | p.12 |
| R-3.9 | The Distance Rule applies to all setup settlement placements: stay at least two edges away from all other settlements. | p.12 |
| R-3.10 | **Starting resources.** Each player takes 1 matching resource card from the supply for each hex adjacent to their **second** settlement. Kept **hidden in hand**. | p.12 |
| R-3.11 | Setup placement is the **only** time a settlement may be placed without connecting to one of your own roads. | FAQ |
| R-3.12 | **Red-number adjacency** (6 and 8) on a randomly generated board is controlled by a game option, **enabled by default**: when on, generated boards where two red numbers are adjacent are rejected or repaired. The 5th Edition requires this; the 6th Edition rulebook does not state it. | `HOUSE` D-6 |

### 2.4 Turn structure (R-4)

| ID | Rule | Source |
|---|---|---|
| R-4.1 | Play proceeds in turns, starting with the first player, clockwise around the table. | p.6 |
| R-4.2 | A turn consists of exactly two phases in order: 1. Production phase, 2. Action phase. | p.6 |
| R-4.3 | After finishing the Action phase, if the player has not won, they pass the dice to the player on their left, who begins their Production phase. | p.6 |

### 2.5 Production phase (R-5)

| ID | Rule | Source |
|---|---|---|
| R-5.1 | **(Optional) Play a development card** before rolling the dice. | p.6 |
| R-5.2 | **Roll dice.** Roll both dice and add them. The total determines which hexes produce this turn. | p.6 |
| R-5.3 | **Production.** Every hex whose number disc matches the roll produces. Each player with a **settlement** on a producing hex receives 1 resource card of that hex's type from the supply. | p.6 |
| R-5.4 | A player with 2 or 3 settlements on the same producing hex receives 1 card per settlement. | p.6 |
| R-5.5 | A player receives **2** resource cards for each of their **cities** on a producing hex. | p.6 |
| R-5.6 | **Supply shortage.** If there are not enough cards of a produced resource in the supply to satisfy everyone's production, **no one** receives any of that resource. Exception: if only **one** player is affected, that player receives as many of those cards as remain in the supply. | p.6 |
| R-5.7 | Terrain → resource mapping: forest → wood, hills → brick, pasture → wool, mountains → ore, fields → wheat, desert → nothing. | p.6 |
| R-5.8 | A hex occupied by the robber does **not** produce resources when its number is rolled. Other hexes of the same type — including ones bearing the same number — still produce normally. | p.6, FAQ |

### 2.6 Rolling a 7 (R-6)

| ID | Rule | Source |
|---|---|---|
| R-6.1 | On a roll of 7, **no hex produces** any resources. | p.6 |
| R-6.2 | **Discard.** Each player (all players, not just the active one) holding **more than 7** resource cards must choose half of them, rounded down, and return them to the supply. *(Example: 9 cards in hand → discard 4.)* | p.6 |
| R-6.2a | Discards resolve **simultaneously** — all affected players choose at once, and play continues when the last confirms. | `HOUSE` D-2 |
| R-6.3 | **Activate the robber.** The active player **must** move the robber to a **different** hex — leaving it in place is not a legal option. Any other land hex is a legal destination, including the desert. | p.6, FAQ, ALM |
| R-6.4 | The active player then steals 1 **random** resource card from an **opponent** who has a building on the robber's new hex. If several opponents have buildings there, the active player chooses which one to rob. The victim holds their cards facedown; the card is taken at random, unseen. | p.6, ALM |
| R-6.5 | Development cards are never stolen by the robber, and do not count toward the discard threshold in R-6.2. | p.9 |
| R-6.6 | No trading is allowed between the dice roll and the completion of the discard + robber resolution. The active player continues their turn normally afterwards. | FAQ |

### 2.7 Action phase — Trade (R-7)

| ID | Rule | Source |
|---|---|---|
| R-7.1 | Actions may be taken as often as desired and in any order, as long as the player has the resources. | p.7 |
| R-7.2 | The active player may trade freely with other players and with the supply. | p.7 |
| R-7.3 | During a player's turn, other players may only trade **with the active player** — not with each other and not with the supply. Triangular (three-way) trades are forbidden. | p.7, FAQ |
| R-7.4 | **Player trade.** The active player announces which resource(s) they want and which they offer. Other players may accept, counteroffer, or make their own proposals. | p.7 |
| R-7.5 | **No gifting.** Cards may not be given away in any form. This includes trading matching resource types (e.g., 3 ore for 1 ore is not allowed). A trade must involve giving *and* taking resources — nothing may be traded for nothing, for a service, or for a promise (no credit or deferred trades). | p.7, FAQ |
| R-7.6 | **General supply trade (4:1).** Put 4 identical resource cards into the supply and take 1 card of a **different** resource. | p.7 |
| R-7.7 | **3:1 port trade.** With a building on a 3:1 port, put 3 identical resource cards into the supply and take 1 card of a **different** resource. | p.7 |
| R-7.8 | **2:1 port trade.** With a building on a 2:1 port, put 2 cards of the resource shown on that port into the supply and take 1 card of a **different** resource. | p.7 |
| R-7.9 | Port access requires the player's own building (settlement or city) on that port's intersection. A player may never use an opponent's port. | p.7, FAQ |
| R-7.10 | Development cards may not be traded or given away. Bonus tiles are likewise never transferable by trade — they move only by meeting their own conditions (R-10). | p.9, *inferred* |
| R-7.11 | Trades must be public: no secret trades, and no player may be required to reveal their hand. | FAQ |
| R-7.12 | The robber never blocks trading, including port trades. | FAQ |
| R-7.13 | **Player trades have no ratio restriction.** Any number of cards and any mix of types may be exchanged for any other, subject only to R-7.5. There is no requirement that the counts match or that either side be a single type. | *inferred from p.7 "resource(s)"* |
| R-7.14 | No player is ever obliged to trade or to accept an offer, and the active player is not bound by an announced offer until the exchange is made. | *inferred* |
| R-7.15 | Trading is confined to the Action phase. No trade may occur before the dice roll, nor during discard/robber resolution (R-6.6). | p.7, *inferred* |
| R-7.16 | Maritime trades (4:1, 3:1, 2:1) may be performed as often as the player can pay for them, and are always available regardless of ports — only the improved ratios require a port. | p.7, ALM |
| R-7.17 | **Empty supply stack.** A maritime trade is illegal unless the target stack can supply the full amount taken. An Invention card takes as many of the requested cards as remain (possibly 1 or 0). | `HOUSE` D-3 |
| R-7.18 | **No type overlap.** No resource type may appear on both the give and take side of a single trade — a generalization of R-7.5 covering multi-type offers. | `HOUSE` D-4 |
| R-7.19 | **Open-market offers.** Multiple trade offers may be live simultaneously, and any player may accept any live offer during the active player's turn. Every offer must still have the active player as one party (R-7.3). Acceptances resolve atomically, first-come, re-validating both parties' holdings at execution; offers invalidated by an intervening state change are rejected with a reason, never executed against stale state. | `HOUSE` D-5 |

### 2.8 Action phase — Build (R-8)

Building costs (player aid, *image-derived* iconography):

| Structure | Cost | VP |
|---|---|---|
| Road | 1 brick + 1 wood | 0 |
| Settlement | 1 brick + 1 wood + 1 wool + 1 wheat | 1 |
| City | 2 wheat + 3 ore | 2 |
| Development card | 1 wool + 1 wheat + 1 ore | ? (0 or 1) |

| ID | Rule | Source |
|---|---|---|
| R-8.1 | To build, return the required resource cards from hand to the supply. | p.8 |
| R-8.2 | **Roads** are placed on empty hex edges — one road per edge. A new road must connect to one of the player's own existing roads, settlements, or cities. Coastal edges are legal. | p.8, FAQ, ALM |
| R-8.3 | A road may not be built starting on the far side of an **opponent's building** — an opponent's settlement or city blocks continuation through that intersection. | p.8, FAQ |
| R-8.4 | **Settlements** are placed on empty intersections, must satisfy the Distance Rule, and must connect to at least one of the player's own roads (after setup). Any point where three hexes meet is a legal intersection, including coastal points without a port. | p.8, FAQ |
| R-8.5 | **Distance Rule.** When placing a settlement, stay at least two edges away from all other buildings (own and opponents'). Equivalently: every building must remain surrounded by three unoccupied intersections, for the whole game. | p.8, FAQ |
| R-8.6 | A player has 5 settlement pieces; to build further settlements, one must first be upgraded to a city. Piece pools are hard caps: 5 settlements, 4 cities, 15 roads. | p.8, FAQ |
| R-8.7 | **Cities always replace settlements.** Remove one of your settlements from the board, return it to your player area, and place the city on that intersection. A city may never be built on an empty intersection. | p.9, FAQ |
| R-8.8 | A player has 4 cities and may not build more. | p.9 |
| R-8.9 | **Development cards** are bought by drawing the top card of the facedown deck. A player may buy as many per turn as they can pay for. | p.9, FAQ |
| R-8.10 | If the development card deck runs out, no more development cards may be built. Development cards never return to the supply. | p.9 |
| R-8.11 | Buildings may never be relocated. A settlement removed by a city upgrade returns to the player's pool and may be rebuilt later at a legal intersection. | FAQ |
| R-8.12 | The robber does not block building. | FAQ |
| R-8.13 | Intersections cannot be reserved — a player may not claim a spot without building on it. | FAQ |

### 2.9 Development cards (R-9)

| ID | Rule | Source |
|---|---|---|
| R-9.1 | Development cards stay **hidden** until played. | p.9 |
| R-9.2 | Development cards do **not** count toward hand size when a 7 is rolled, and cannot be stolen by the robber. | p.9 |
| R-9.3 | A player may play **at most 1** development card per turn, placing it **face up** in their player area. | p.9 |
| R-9.4 | A development card may not be played on the turn it was bought. | p.9 |
| R-9.5 | A development card may be played either **before rolling the dice** or at any time during the Action phase — including in the middle of trading. Playing one before the roll consumes the turn's single card play. | p.6, p.9, FAQ |
| R-9.6 | Development cards may not be traded or given away, and never go back into the supply. | p.9 |

**Card effects**

| ID | Card | Effect | Source |
|---|---|---|---|
| R-9.7 | **Knight** (14×) | Activate the Robber (R-6.3, R-6.4): move the robber to a different hex and steal 1 random resource card from an opponent with a building on that hex. Played Knights remain face up for the rest of the game. | p.9 |
| R-9.8 | **Invention** (2×) | Take any 2 resource cards from the supply into hand — 2 of the same or 2 different resources. (Called *Year of Plenty* in earlier editions.) | p.9 |
| R-9.9 | **Monopoly** (2×) | Announce **one** resource type; every other player must give you all their resource cards of that type. Only one type may be named, regardless of how many cards are received. Players are not required to reveal their hands — the rules assume honesty. | p.9, FAQ |
| R-9.10 | **Road Building** (2×) | Build 2 roads at no cost (no resources spent). Normal road placement rules apply. | p.9 |
| R-9.10a | — | If fewer than 2 legal road placements exist (blocked board or road pool short), place as many as are legal (1 or 0); the card is still discarded and the turn's development-card allowance is still consumed. | `HOUSE` D-1 |
| R-9.11 | **Victory Point** (5×) | Worth 1 VP. Must be kept **hidden** in the player area unless revealing them reaches the VP total needed to win; then reveal all VP cards at once, including those built this turn. | p.9, p.2, FAQ |
| R-9.12 | **VP card exception.** Any number of VP cards may be played, even on the turn they were bought, in order to win — this bypasses R-9.3 and R-9.4. | p.2, p.9 |

### 2.10 Bonus tiles (R-10)

| ID | Rule | Source |
|---|---|---|
| R-10.1 | **Longest Route** (2 VPs). The first player with **5 continuous roads** in play receives the tile. | p.8 |
| R-10.2 | If another player has **more** continuous roads in play, they immediately receive the tile. | p.8 |
| R-10.3 | A route is a continuous path of road segments connecting two intersections, not interrupted by another player's pieces. **Forks do not add length** — only the longest single path counts. Your own settlements and cities do **not** interrupt your route; an opponent's building does. Closed loops are possible and count as their individual segments. | p.8, FAQ, ALM |
| R-10.4 | A route can be **broken** by an opponent building a settlement on an intersection within it, splitting it into two segments. All normal building rules must be observed to do this. | p.8, FAQ |
| R-10.5 | If a player's route is broken such that they no longer meet the requirement, the tile returns to the supply and stays there until a **single** player has the longest continuous route of at least 5 roads; that player immediately receives the tile and its 2 VPs. If the current holder still qualifies, they keep it. | p.8, FAQ |
| R-10.6 | **Ties do not transfer.** A tile passes only to a player with strictly *more* than the current holder; on a tie the tile stays with its current owner. | FAQ |
| R-10.7 | A broken route may be repaired by building a "bypass" — a detour around the blocking building. | FAQ |
| R-10.8 | **Largest Army** (2 VPs). The first player with **3 Knight cards in play** receives the tile. If another player has more Knights in play, they immediately receive it. Only *played* (faceup) Knights count; Knights in hand count for nothing. | p.9, FAQ |

### 2.11 Winning (R-11)

| ID | Rule | Source |
|---|---|---|
| R-11.1 | If a player has 10 or more VPs at any point **during their own turn**, the game ends immediately and they win. A player can never win during another player's turn. | p.10, FAQ |
| R-11.2 | To claim victory, the player turns over any number of Victory Point cards, including ones built that turn, to demonstrate reaching 10 VPs. | p.10 |
| R-11.3 | VP tally components: settlements (1 each), cities (2 each), Longest Route tile (2), Largest Army tile (2), VP cards (1 each), roads (0), Knights played (0 in themselves). | p.10 |
| R-11.4 | Victory is immediate and cannot be declined or deferred. If the winning player does not notice, the other players should tell them — a won game cannot be taken back. | FAQ |

---

## 3. Materials — inventory

### 3.1 Board & terrain

| # | Item | Qty | Detail |
|---|---|---|---|
| M-01 | Sea frame pieces | 6 | Puzzle-piece ends with matching numbers; carry the ports |
| M-02 | Ports (printed on frame) | 9 *(image-derived; corroborated by the 5th Ed. 9 harbour pieces)* | 4× 3:1 generic; 5× 2:1 (one each: brick, wood, wool, wheat, ore) |
| M-03 | Terrain hexes | 19 | 4× forest, 4× pasture, 4× fields, 3× hills, 3× mountains, 1× desert |
| M-04 | Number discs | 18 | 1×2, 2×3, 2×4, 2×5, 2×6, 2×8, 2×9, 2×10, 2×11, 1×12. 6 and 8 printed in red. Backs lettered A–R for the variable setup |
| M-05 | Robber | 1 | Grey/neutral figure |

### 3.2 Cards

| # | Item | Qty | Detail |
|---|---|---|---|
| M-06 | Resource cards | 95 | 19× each of brick, wood, wool, wheat, ore. Shared card back |
| M-07 | Development cards | 25 | 14× Knight, 5× Victory Point, 2× Monopoly, 2× Road Building, 2× Invention. Shared card back |

### 3.3 Player pieces (4 colors: blue, red, white, orange)

| # | Item | Qty | Per player |
|---|---|---|---|
| M-08 | Settlements | 20 | 5 per color |
| M-09 | Cities | 16 | 4 per color |
| M-10 | Roads | 60 | 15 per color |

### 3.4 Tiles, accessories & derived items

| # | Item | Qty | Detail |
|---|---|---|---|
| M-11 | Longest Route tile | 1 | Bonus VP tile, 2 VPs |
| M-12 | Largest Army tile | 1 | Bonus VP tile, 2 VPs |
| M-13 | Dice | 2 | Standard d6 (one red, one yellow) |
| M-14 | Player aids | 4 | Front: building costs; back: turn overview + development card rules |
| M-15 | Card trays | 2 | 6 slots total: 5 resource stacks + 1 development deck |
| M-16 | Rulebook | 1 | Not a play component |

---

## 4. Visibility & information-state model

### 4.1 Visibility taxonomy

| Code | Name | Meaning |
|---|---|---|
| `PUBLIC` | Visible to all | Face-up / on the board; every player has full knowledge |
| `OWNER` | Visible to owner only | Held privately; other players know the *count* but not the identity |
| `HIDDEN` | Unknown to all | Facedown, no player has knowledge (e.g., deck order) |
| `COUNTABLE` | Public quantity, private identity | Others may legitimately observe how many, not what |
| `TRANSIENT` | Momentarily revealed | Exposed during a specific action, then returns to a prior state |
| `DERIVED` | Computed, not a physical item | Game state inferred from other items (e.g., VP totals) |

**Key asymmetry to model:** a player's resource hand is `OWNER` for identity but `COUNTABLE` for size — hand size must be publicly known because the discard-on-7 rule (R-6.2) depends on it, and a player must answer honestly how many cards they hold. Development cards in hand are also `COUNTABLE`/`OWNER`, but are explicitly excluded from the 7-discard count (R-9.2).

### 4.2 Per-item state table

| Item | Location states | Visibility states | Notes |
|---|---|---|---|
| **Sea frame piece** (M-01) | assembled in frame | `PUBLIC` | Static after setup. Arrangement fixed (R-2.1) or randomized (R-3.1) |
| **Port** (M-02) | on frame | `PUBLIC` | Access = own building on the port intersection; access status is public |
| **Terrain hex** (M-03) | in frame slot | `PUBLIC` (faceup once placed) | Momentarily `HIDDEN` while being drawn randomly in variable setup (R-3.2) |
| **Number disc** (M-04) | on a hex / off-board (desert has none) | `HIDDEN` (facedown, letter side up during variable setup) → `PUBLIC` (number faceup) | Variable setup explicitly uses a facedown ordered phase (R-3.3) |
| **Robber** (M-05) | on exactly one hex (starts on desert) | `PUBLIC` | Always public; blocks production on its hex (R-5.8) but not building or trade (R-7.12, R-8.12) |
| **Resource card** (M-06) | supply stack / player hand / in-transit during trade / discarded to supply | supply: `PUBLIC` (faceup stacks, count visible) · hand: `OWNER` + `COUNTABLE` · trade: `TRANSIENT` (revealed to the trade partner and, being a public trade, to the table) · stolen: `TRANSIENT` to the thief only, never revealed to the table | Supply counts are public and may be checked before distribution (R-5.6). Steals are random and unseen (R-6.4). Monopoly forcibly reveals part of every hand for one resource type (R-9.9) |
| **Development card** (M-07) | facedown deck / player hand (unplayed) / player area (played, faceup) / VP card (held hidden until win) | deck: `HIDDEN` (order unknown to all; remaining count `PUBLIC`) · hand: `OWNER` + `COUNTABLE` · played: `PUBLIC` · VP card: `OWNER` until win, then `PUBLIC` | Never returns to supply (R-9.6). Played Knights stay faceup and are publicly counted for Largest Army (R-10.8) |
| **Settlement** (M-08) | player pool (unbuilt) / on intersection / returned to pool on city upgrade | `PUBLIC` in all states | 5 per player, hard cap (R-8.6) |
| **City** (M-09) | player pool / on intersection | `PUBLIC` | 4 per player, hard cap (R-8.8); always replaces a settlement (R-8.7) |
| **Road** (M-10) | player pool / on edge | `PUBLIC` | 15 per player; never removed once placed |
| **Longest Route tile** (M-11) | unowned near board / held by a player | `PUBLIC` | Returns to the unowned pool when a route breaks and no single player qualifies (R-10.5) |
| **Largest Army tile** (M-12) | unowned near board / held by a player | `PUBLIC` | Transfers only on strictly more Knights (R-10.6, R-10.8); can never return to the unowned pool once claimed |
| **Dice** (M-13) | idle / rolled | roll result `PUBLIC` | Also used to determine the first player (R-2.8) |
| **Player aid** (M-14) | with each player | `PUBLIC` (reference only) | No game state |
| **Card tray** (M-15) | table | `PUBLIC` | Organizational only |

### 4.3 Derived state (`DERIVED`)

| Item | Visibility | Notes |
|---|---|---|
| Player VP total | Partly public | Buildings and tiles are `PUBLIC`; hidden VP cards make the true total private until revealed (R-9.11) — track *apparent* and *actual* VP separately |
| Longest continuous route per player | `PUBLIC` | Computed from public road/building positions, forks excluded, opponent buildings breaking (R-10.3) |
| Knights played per player | `PUBLIC` | Faceup in player areas |
| Resource hand size per player | `PUBLIC` | Required for the 7-discard rule (R-6.2); players must answer honestly |
| Development-card hand size per player | `PUBLIC` | Excluded from the discard count (R-9.2) |
| Supply stack counts per resource | `PUBLIC` | Drives the shortage rule (R-5.6) |
| Port access per player | `PUBLIC` | Derived from own buildings on port intersections |
| Production map (number → hexes → adjacent buildings) | `PUBLIC` | Recomputed as buildings are placed |

### 4.4 Hidden-information summary

Exactly four things are hidden in this game:

1. **Resource card identities in hands** — counts public, identities private.
2. **Unplayed development card identities in hands** — counts public, identities private.
3. **Development deck order** — unknown to all.
4. **Number disc values during the variable-setup placement step** — a temporary hidden state (R-3.3).

Everything else — board, buildings, roads, robber, played cards, tiles, supply stacks, dice — is fully public. For a digital build this means the server needs to hide very little, but the two card-identity cases must never leak (including via animation, timing, or message size).

---

## 5. Digital implementation model

### 5.1 Board topology

- 19 hex slots, **54 intersections**, **72 edges**. Recommend axial or cube coordinates for hexes, with intersections and edges derived from hex corners/sides so that adjacency queries (`hex→intersections`, `intersection→edges`, `edge→intersections`, `intersection→hexes`) are all precomputed lookups.
- Ports attach to a **pair of intersections** on the frame, not to a hex. Port access is a property of an intersection.
- The desert never carries a number disc (R-3.3) and never produces (R-5.7).

### 5.2 State schema (zones)

| Zone | Contents | Visibility |
|---|---|---|
| `board.hexes[19]` | terrain type, number disc, robber flag | `PUBLIC` |
| `board.intersections[54]` | building (none/settlement/city) + owner, port ref | `PUBLIC` |
| `board.edges[72]` | road + owner | `PUBLIC` |
| `supply.resources{5}` | remaining count per resource type | `PUBLIC` |
| `supply.devDeck` | ordered list of remaining development cards | `HIDDEN` (count `PUBLIC`) |
| `tiles` | holder of Longest Route / Largest Army (or unowned) | `PUBLIC` |
| `players[n].hand.resources` | multiset of resource cards | `OWNER`, size `PUBLIC` |
| `players[n].hand.devCards` | list of unplayed development cards, each with `boughtOnTurn` | `OWNER`, size `PUBLIC` |
| `players[n].played.knights` | count of faceup Knights | `PUBLIC` |
| `players[n].played.vpCards` | count of VP cards (revealed only on win) | `OWNER` until win |
| `players[n].pool` | unbuilt settlements / cities / roads | `PUBLIC` |
| `turn` | active player, phase, dice roll, `devCardPlayedThisTurn` flag, pending sub-states | `PUBLIC` |

Per-development-card `boughtOnTurn` is required to enforce R-9.4; a single per-turn boolean enforces R-9.3.

### 5.3 Turn state machine

```
SETUP_ROUND_1  → (per player: place settlement → place road)        [R-3.7]
SETUP_ROUND_2  → (reverse order: place settlement → place road
                  → collect starting resources)                     [R-3.8, R-3.10]
      ↓
PRE_ROLL       → optional PLAY_DEV_CARD                             [R-5.1, R-9.5]
      ↓ ROLL_DICE
   roll == 7 ?
      ├─ yes → DISCARD_PENDING (all players with >7 cards)          [R-6.2]
      │         → ROBBER_PLACEMENT → STEAL_CHOICE                   [R-6.3, R-6.4]
      └─ no  → DISTRIBUTE_PRODUCTION (with shortage check)          [R-5.3–R-5.6]
      ↓
ACTION         → any number of: TRADE / BUILD / BUY_DEV_CARD
                 / PLAY_DEV_CARD (if not yet played this turn)      [R-7, R-8, R-9.5]
      ↓ (victory check after every state change on the active player's turn)
END_TURN → next player's PRE_ROLL      |      GAME_OVER             [R-11.1]
```

Playing a Knight in `PRE_ROLL` and then rolling a 7 activates the robber **twice** in one turn — this is correct, not a bug (R-9.7 + R-6.3).

### 5.4 Action catalogue

| Action | Preconditions | Notes |
|---|---|---|
| `PLACE_SETUP_SETTLEMENT(v)` | setup phase, `v` empty, Distance Rule | No road connection required (R-3.11) |
| `PLACE_SETUP_ROAD(e)` | setup phase, `e` empty, adjacent to the settlement just placed | |
| `ROLL_DICE` | phase = `PRE_ROLL` | Two d6, server-side RNG |
| `DISCARD(cards)` | `DISCARD_PENDING`, player has >7, `len(cards) == floor(n/2)` | All qualifying players; resolve simultaneously |
| `MOVE_ROBBER(hex)` | robber sub-state, `hex != current robber hex` | Desert allowed (R-6.3) |
| `CHOOSE_VICTIM(player)` | ≥2 opponents with buildings on the robber hex | Skipped when 0 or 1 candidate |
| `BUILD_ROAD(e)` | `e` empty, connects to own road/building, not through an opponent's building, road pool > 0, cost payable | R-8.2, R-8.3, R-8.6 |
| `BUILD_SETTLEMENT(v)` | `v` empty, Distance Rule, connects to own road, settlement pool > 0, cost payable | R-8.4–R-8.6 |
| `BUILD_CITY(v)` | `v` holds own settlement, city pool > 0, cost payable | Returns the settlement to the pool (R-8.7) |
| `BUY_DEV_CARD` | deck non-empty, cost payable | Unlimited per turn (R-8.9) |
| `PLAY_DEV_CARD(card, params)` | not bought this turn, none played this turn, own turn | VP cards exempt (R-9.12) |
| `TRADE_SUPPLY(give, take)` | ratio 4:1, or 3:1/2:1 with port access; `give` uniform; `take != give` type | R-7.6–R-7.9 |
| `PROPOSE_TRADE(give, want)` / `ACCEPT` / `REJECT` / `COUNTER` / `WITHDRAW` | active player is one party; both sides non-empty; no resource type on both sides (R-7.18); both parties hold what they offer **at the moment of resolution**, not at proposal time. Multiple offers may be live at once (R-7.19) | R-7.3–R-7.5, R-7.13, R-7.18, R-7.19 |
| `END_TURN` | phase = `ACTION`, no pending sub-state | R-4.3 |

### 5.5 Validation invariants

Cheap assertions worth running after every state transition:

1. Total cards of each resource type = 19 (supply + all hands). Total development cards = 25 (deck + hands + played).
2. Per player: settlements on board + in pool = 5; cities = 4; roads = 15.
3. Every road connects to a road, settlement, or city of the same owner (transitively, back to a setup piece).
4. No two buildings within one edge of each other (Distance Rule holds continuously, R-8.5).
5. At most one road per edge, at most one building per intersection.
6. Exactly one robber, always on a land hex.
7. Each bonus tile is held by at most one player; Largest Army requires ≥3 played Knights and Longest Route requires ≥5 continuous roads, in both cases strictly more than every other player.
8. `devCardPlayedThisTurn` ≤ 1 (excluding VP reveals).

### 5.6 Authority & randomness

Four randomness sources must be server-side and unforgeable: dice rolls, development deck shuffle, the random steal (R-6.4), and — in the Variable Setup — hex and frame randomization. The steal is the one place where a card moves between private zones without either party choosing it; the victim must not learn what left their hand until they inspect it, and no other player may learn it at all.

### 5.7 Rules that are easy to get wrong

- **Supply exhaustion** (R-5.6): all-or-nothing, *except* when exactly one player is affected. Check before distributing.
- **Robber must move** (R-6.3): "stay put" is never legal; the desert is a legal destination.
- **Two robber entry points** (R-6.3, R-9.7) share one resolution routine, and can both fire in the same turn.
- **Forks don't count** toward route length (R-10.3), and your own buildings don't break your route.
- **Ties never transfer a bonus tile** (R-10.6).
- **Victory is only checked on the active player's turn** (R-11.1) — losing or gaining a tile during someone else's turn cannot end the game.
- **Trade is turn-gated** (R-7.3) and must be a genuine two-sided exchange (R-7.5). The trade *protocol* — offer lifecycle, binding, concurrency — has no rulebook basis at all and is a house ruling (R-7.19, open market), with the concurrency consequences in §9.1.
- **Cities replace settlements** (R-8.7); the freed settlement returns to the pool and is reusable.

---

## 6. Engine architecture

### 6.1 Principle: library-first, not service-first

The engine is a **pure Rust library** — no I/O, no async, no networking in the core. Every consumer is a thin adapter around it.

This matters most for the AI-training use case. A single engine step should land in the low single-digit **microseconds**; an HTTP round-trip costs tens to hundreds of microseconds at minimum. Putting a network boundary on the training hot path would make serialization and syscalls dominate wall-clock time and render the language choice underneath irrelevant. "API first" here means *stable, well-specified interface* — the action catalogue in §5.4 — not *service first*.

### 6.2 Crate layout

| Crate | Responsibility | Depends on |
|---|---|---|
| `catan-core` | State, rules, legal-move generation, action application, event emission. Pure, deterministic, no I/O | nothing |
| `catan-replay` | Log read/write, replay driver, snapshot & seek, per-seat redaction | core |
| `catan-py` | PyO3 bindings: batched environments, observation encoding, action masks | core |
| `catan-server` | HTTP/WS service, matchmaking, persistence | core, replay |
| `catan-wasm` | Browser bindings for the client | core, replay |
| `catan-analytics` | Parquet/Arrow export, derived-event materialization | replay |

The dependency direction is strictly one-way: everything depends on `catan-core`, and `catan-core` depends on nothing. If the core ever needs a network or database type, the design has gone wrong.

### 6.3 State representation

- **Fixed-size and `Copy`.** From the §5.2 zones — 19 hexes, 54 intersections, 72 edges, ≤4 players, pools, hands, deck, turn flags — the whole state is on the order of **~300 bytes** with no heap allocation. Cloning a state for an MCTS node is a `memcpy` of a few cache lines, effectively free.
- **Bitboards.** 72 edges fit in a `u128` and 54 intersections in a `u64`, one per player. Legal road generation becomes `expand(own_roads) & !occupied & !blocked_by_opponents` — a handful of bit operations rather than a graph walk. This is where the order of magnitude over a scripting language comes from.
- **Enum state machine.** The §5.3 phases and §5.4 actions are an enum-and-`match` problem. Exhaustive matching means adding a phase without handling a transition is a compile error, which is the right failure mode for a rules engine whose bugs otherwise manifest as subtly illegal states.
- **Hot spot: longest route** (R-10.3). The only nontrivial algorithm in the game, run on every road placement, made a genuine path search by the forks-don't-count rule. Design it incremental or memoized from the start; it will dominate the profile otherwise.

### 6.4 Determinism

Determinism is required for reproducible training, debuggable replays, and the snapshot-verification invariant in §7.6.

- **Split RNG streams** — separate seeded generators for dice, deck shuffle, the random steal (R-6.4), and setup randomization. Holding one fixed while varying another is essential when debugging and when running paired evaluations.
- **No incidental nondeterminism.** `std::HashMap` randomizes its seed per process; use `BTreeMap` or a fixed hasher anywhere iteration order can reach game state.
- **Version stamping.** Every game records `engine_version` and `rules_version`. With six `HOUSE` rules and four inferred rules outstanding, the rules *will* change, and old data must remain interpretable.

### 6.5 AI training interface

- **Batched environments.** Crossing the PyO3 boundary once per step costs more than the step itself. Step *N* games per call, EnvPool-style, with observations written directly into caller-provided numpy buffers.
- **Action masks in Rust.** RL needs a legal-action mask every step; generating it in Python would negate the engine's speed. This is a primary consumer of the bitboard representation.
- **Per-seat observations** are generated from the §4 visibility model — the same classification that drives replay redaction (§7.3). One implementation, two consumers.
- **Determinization for imperfect-information search.** Catan is not a perfect-information game, so tree search needs opponents' hidden state resampled consistently with public history. The four items in §4.4 are exactly and completely what must be resampled — that list is the specification.
- **Trade mode is configurable.** D-5's open market gives an unbounded, combinatorial trade action space *and* lets non-active players act, which breaks the clean turn-based MDP most RL machinery assumes. Published Catan RL work generally disables or heavily restricts trading. The engine therefore exposes trade policy as a dimension — `full` (open market, for human play), `restricted` (a small fixed offer menu), `disabled` — rather than hard-coding R-7.19. **Build this seam now; retrofitting it later means touching every layer.**

### 6.6 Performance targets

Targets to validate with a benchmark harness — **not measurements**, and nothing here should be quoted as fact until benchmarked:

| Metric | Target |
|---|---|
| Single action application | low single-digit µs |
| Full random game (setup → win) | sub-millisecond |
| Self-play throughput | millions of steps/sec across cores via rayon |
| State clone (MCTS node) | ~memcpy of ~300 bytes |
| Batched env step, N=1024 | FFI overhead amortized below per-step cost |

---

## 7. Game history, replay & data model

*(Recording and storage. The metrics computed from this data are §10.)*

Every game can be recorded in full: a complete, ordered event log that serves as the source material for replays *and* as the raw data for statistics at game, player, and cross-game scope.

### 7.1 Recording decisions

| # | Decision | Choice |
|---|---|---|
| H-1 | Log content | **Actions + resolved randomness.** Every action plus the concrete outcome of each random event, with periodic state snapshots for seeking |
| H-2 | Hidden information | **Omniscient log, redacted on serve.** Full truth stored; per-seat fog applied at serve time |
| H-3 | Recording scope | **Configurable per session.** Default on for served games, off for self-play unless requested |
| H-4 | Negotiation churn | **Record everything** — proposals, counteroffers, rejections, withdrawals |
| H-5 | Storage | **Object store + Parquet exports.** Compact binary logs in S3-compatible storage; periodic columnar exports for analytics |
| H-6 | Identity | **Every seat carries a durable ID**, with agents identified by name + version |
| H-7 | Derived events | **Separate regenerable stream.** The primitive log is canonical; derived events are a materialized view |

**Why H-1 matters most.** Recording resolved randomness rather than just a seed decouples stored games from any single engine build. A seed-only log requires bit-exact determinism *forever* — and with six `HOUSE` rules and four unverified inferences still in play, a rules correction would silently reinterpret every historical game rather than failing loudly. Explicit outcomes make replay a pure fold over data.

### 7.2 Event model

Every event shares an envelope:

| Field | Purpose |
|---|---|
| `game_id`, `seq` | Identity and total ordering |
| `wall_time`, `mono_time` | Timestamps for pacing on replay and think-time analytics |
| `actor` | Seat index + durable player ID (H-6), or `system` |
| `type`, `payload` | Discriminated union of the categories below |
| `visibility` | A §4.1 class, attached at emission so redaction is mechanical |

Categories:

1. **Lifecycle** — `GameCreated` (rules_version, engine_version, options incl. R-3.12 and trade mode, seat assignment, seeds), `GameEnded` (winner, final VP breakdown).
2. **Setup** — placements from R-3.7/R-3.8, board generation result.
3. **Decisions** — the §5.4 action catalogue: builds, buys, dev card plays, robber moves, discards, end turn.
4. **Randomness resolution** — `DiceRolled{d1,d2}`, `DevCardDrawn{card}`, `CardStolen{resource, from, to}`, `BoardGenerated{layout}`. Recording the *outcome*, per H-1.
5. **Negotiation** — `TradeProposed`, `TradeCountered`, `TradeRejected`, `TradeWithdrawn`, `TradeAccepted`. Non-state-changing but recorded in full per H-4; under R-7.19's open market this is most of the player interaction in the game.
6. **Snapshots** — `StateSnapshot` every *N* events, carrying a state checksum.

### 7.3 Replay and redaction

Replay is `fold(apply, events)` from a snapshot or from the start. Snapshots give seeking without replaying from event zero.

Redaction derives entirely from §4: `PUBLIC` data is served to everyone, `OWNER` data only to its owner, `HIDDEN` data to no one. The four items in §4.4 are exactly what must be masked.

Two things about redaction are easy to get wrong:

- **It is a function of `(event, viewer, time)`, not a static classification.** A card that is `OWNER` when drawn becomes `PUBLIC` when played; VP cards are hidden until the winning reveal (R-9.11); Monopoly forcibly exposes part of every hand (R-9.9). A redaction layer that classifies by event type alone will either leak or over-hide.
- **It must run server-side, before serialization.** Never ship a full log to a client and filter in the UI. Live spectating and mid-series replay sharing both depend on this path being correct, which makes it security-critical rather than merely cosmetic.

### 7.4 Analytics scopes

The log is the source of truth; everything below is derived and regenerable (H-7).

| Scope | Examples |
|---|---|
| Per game | Length, VP progression, dice distribution, robber activity |
| Per player, across games | Win rate, build-order tendencies, negotiation behaviour, resource efficiency |
| Per seat / turn order | First-player advantage, position effects |
| Per agent version | v3 vs v4 head-to-head across thousands of games (H-6) |
| Per rules version / options | Effect of the R-3.12 red-number option, trade mode comparisons |
| Per board / setup variant | Fixed vs Variable outcomes, layout-driven imbalance |
| Within-game temporal | Per-turn and per-phase slices, think time |

A sketch for the Parquet layer: `dim_game` (rules_version, engine_version, options, setup variant, board hash), `dim_player` (identity, human/agent, agent version), `fact_game_player` (seat, colour, final VP breakdown, win flag), `fact_events`, `fact_turns`. All regenerated from the canonical logs, never hand-maintained.

**Aggregation hazard:** because `HOUSE` rules and options like R-3.12 change actual gameplay, games are not homogeneous. Every aggregate must be filterable by `rules_version` and game options, or analyses will silently mix incomparable games. Make those columns mandatory rather than nullable.

### 7.5 Sizing

Rough estimates to validate, not measurements. A game runs a few hundred state-changing actions; H-4's negotiation churn could multiply total event count several-fold under an open market. At tens of bytes per binary event, expect **tens of KB per game uncompressed, low single-digit KB compressed**, putting a million games in the order of gigabytes — comfortable for object storage.

### 7.6 Risks

1. **Trade churn is unbounded.** R-7.19 lets any player propose at any time during a turn, and H-4 records all of it. A misbehaving client or bot could inflate a log arbitrarily. Rate-limit offers per turn in the engine — this is a log-bloat and denial-of-service vector, not just a tidiness concern.
2. **Redaction leaks are silent.** Add an explicit test asserting that no `OWNER` or `HIDDEN` datum appears in any other seat's serialized view, across a corpus of replayed games. A leak will not otherwise surface until someone exploits it.
3. **Replay divergence.** Verify replayed state against the checksum in each `StateSnapshot`. A mismatch means either a corrupted log or an engine change that altered semantics — both need to fail loudly, immediately.
4. **Rules drift across the corpus.** See the aggregation hazard in §7.4.
5. **Identity and privacy.** H-6 creates durable cross-game player records. Retention, deletion, and pseudonymisation of human identities need a policy before the first human game is recorded, not after.

---

## 8. Platform scope

The engine (§6) is one component of a product: accounts, lobbies, matchmaking, spectating, and chat sit above it. Nothing in this section may leak into `catan-core`.

### 8.1 Decisions

| # | Area | Decision |
|---|---|---|
| P-1 | Guest identity | **Device-persistent and claimable.** A guest carries a durable ID and can later attach email or Google, keeping full history |
| P-2 | Disconnect / abandonment | **Bot takeover after a timeout.** The game continues; the substitution is recorded |
| P-3 | Pacing | **Real-time only** for v1. All players present, turn timers |
| P-4 | Authentication | **Self-hosted open source** (Ory, Keycloak, or self-hosted Supabase) |
| P-5 | Discovery | **Browsable lobby list.** No matchmaking queue in v1 |
| P-6 | Spectators | **Allowed, with fog** — public observer view only |
| P-7 | Communication | **Text chat in v1**; voice designed-for-later, not built |
| P-8 | Chat data | **Recorded in a separate stream**, with its own retention class |

### 8.2 Identity model

Three distinct concepts that are easy to conflate:

- **Principal** — the durable analytics identity. Owned by us, not by the auth provider.
- **Credential** — how a principal proves itself: a device token (guest), email + password, or a Google account. A principal may have several.
- **Seat** — a position in one game (0–3), occupied by an actor for an interval.

**Guest claiming (P-1) must not rewrite game logs.** Logs are immutable (§7). Implement claiming as an alias in an identity table — `guest_principal → account_principal` — and resolve through it at analytics time. The alternative, rewriting historical events to carry the new ID, breaks the snapshot checksums in §7.6 and destroys the append-only property that makes replay trustworthy.

**Own the principal table.** Treat the P-4 auth system as an identity *source*, not the system of record. Guest-to-account linking is the least standardised flow across auth providers, and keeping the principal mapping in our own schema means a provider change never touches analytics history.

**P-2 makes a seat's actor time-varying.** A seat can begin as a human and finish as a bot, so:

- The log carries `SeatActorChanged { seat, from, to, reason }` events.
- `fact_game_player` is keyed per seat-*interval*, not per seat.
- Any per-player aggregate must decide how to treat substituted games. **Default: flag them and exclude from rated statistics**, since neither the departed human nor the substituting bot played a whole game.

Actor types, all carrying a durable ID per H-6: `human-account`, `human-guest`, `bot-heuristic@version`, `bot-llm@model+version`, `bot-trained@version`.

### 8.3 Lobby model

States: `open` (publicly listed) → `private` (invite-only) → `starting` → `in-game` → `closed`.

Lobby configuration is exactly the set of things that make games incomparable in analytics (§7.4), so it is recorded verbatim into `GameCreated`:

- Setup variant — Fixed or Variable (§2.2, §2.3)
- Red-number adjacency option (R-3.12)
- Trade mode — `full` / `restricted` / `disabled` (§6.5)
- Seat count (3–4) and which seats are bots, with difficulty
- Turn timer duration

Notes:

- **Invite links** use unguessable, revocable tokens. A private lobby's security is entirely the token's entropy.
- **Bots fill seats in the lobby** and may be added or removed before start. One human plus two or three bots is a valid game — the rules require 3–4 *players*, not 3–4 humans.
- **Turn timers are mandatory** under P-3. On expiry the turn auto-resolves (forced actions only) or the seat passes to a bot per P-2.
- **Accepted consequence of P-3:** live state may assume all players are present. Adding correspondence play later would mean re-architecting live state handling, not just extending timeouts.

### 8.4 Spectators — and what P-6 costs

Choosing fogged spectating over no spectating **moves §7.3 redaction from a post-game convenience onto the live path, in v1**. Three consequences:

1. **The redaction leak test (§7.6, risk 2) becomes a launch blocker**, not a later hardening task. A bug there now exposes live hands to strangers rather than mis-rendering an old replay.
2. **Spectator view is the neutral observer view**: `PUBLIC` data only, no seat's `OWNER` data, ever. It is not "a player view minus that player" — it is strictly less than any player's view.
3. **Fog does not eliminate collusion.** A spectator can relay timing, board reads, and inference back to a seated player out-of-band. Mitigate with a configurable **broadcast delay** on public games, and consider disabling spectating entirely for rated play once rating exists.

Spectators are not seats: they hold no `player_id` in the game log and never appear in `fact_game_player`.

### 8.5 Chat

Text chat in v1 (P-7), per-lobby and per-game channels. Under R-7.19's open market, negotiation is the core social loop, and structured offers alone would leave it flat.

**Storage (P-8).** Chat lives in its own stream keyed by `game_id` and correlated to the game log by sequence number and timestamp, so replay can interleave conversation with actions without embedding personal data in the canonical corpus. This is what makes a deletion request satisfiable without rewriting immutable game logs — the reason to accept the small cost of correlating two streams.

**Moderation baseline:** per-player mute and block, a report flow, and server-side filtering. The chat log is the evidence trail; that is a substantial part of why it is recorded at all.

**Voice (deferred).** Keep the transport abstraction voice-ready without building it. For ≤4 participants a WebRTC mesh is viable; beyond that, an SFU or a vendor (LiveKit, Daily, Agora). **Voice is not recorded** — the consent, storage, and jurisdiction burden is disproportionate, and none of the analytics goals need it.

### 8.6 Authentication (P-4)

Self-hosted, and must cover: email + password with verification and reset, Google OAuth, guest-to-account claiming, and session tokens usable over WebSocket. Running it ourselves means patching, key rotation, and breach response are ours — budget for that as ongoing work rather than a one-off integration.

---

## 9. Bots and the LLM player

### 9.1 Decisions

| # | Area | Decision |
|---|---|---|
| B-1 | Lineup | **Heuristic + LLM first**; a trained agent later |
| B-2 | LLM output budget | **Adaptive** — index-only by default, brief reasoning on decisions that carry the game |
| B-3 | LLM access | **Internal and flagged accounts only.** Not in public lobbies |
| B-4 | LLM purpose | **All four**: stand-in, live opponent, evaluation baseline, bootstrap training data |

### 9.2 One player interface

Every actor — human, heuristic, LLM, trained agent — implements the same port: *given a redacted observation and the list of legal actions, return one action*. The engine cannot tell them apart. This is what makes B-1's staged lineup cheap, and it is the same interface the batched training envs (§6.5) already need.

Two universal requirements: every bot answers within a bounded time, and **any bot failure or timeout falls back to the heuristic bot** rather than stalling the game.

### 9.3 Heuristic bot — the availability floor

Instant, deterministic, in-process, free. It serves four jobs beyond being an opponent:

- Disconnect takeover (P-2), which must be immediate and must not cost money
- Lobby filling
- Fallback when the LLM errors, times out, or hits a spend cap
- Regression baseline — if a trained agent cannot beat it, something is wrong

Because P-2 and B-3 both depend on it, the heuristic bot is a **prerequisite for launch**, not an optional extra alongside the more interesting LLM work.

### 9.4 LLM player

**Why this fits.** The engine already emits legal-action masks (§6.5). The model selects an *index* from an enumerated list, so an illegal move is structurally impossible — no parsing, no validation, no retry loop. This is the property that makes the idea cheap rather than fiddly.

**Prompt structure, built for caching:**

| Segment | Contents | Volatility |
|---|---|---|
| Static prefix | Rules summary, action-encoding legend, board layout and port positions | Fixed for the whole game — **cache this** |
| Dynamic suffix | Current public state, own hand, enumerated legal actions | Per decision |

The board never changes after setup, so it belongs in the cached prefix rather than being resent hundreds of times. Rough estimates to measure, not facts: static prefix on the order of 1–2k tokens, dynamic suffix a few hundred.

**Cut the call count before optimising the call.** The engine should **auto-resolve forced decisions** — any state with exactly one legal action — without consulting any player. Much of a Catan game is forced or near-forced, so this removes most LLM calls outright, and it benefits RL rollouts identically.

**Where the reasoning budget goes (B-2).** Index-only everywhere except: initial placement (R-3.7, R-3.8), robber placement and victim choice (R-6.3, R-6.4), trade evaluation, and development card timing. These are where Catan games are actually decided; everything else is bookkeeping.

**Trade mode must be `restricted`.** R-7.19's open market makes the legal action list unbounded — all possible offers cannot be enumerated into a prompt. LLM play uses the same `restricted` seam that RL needs (§6.5), which is the second independent reason that seam has to exist before either is built.

**Pin everything for B-4's evaluation role.** Model ID, version, temperature, and a hash of the prompt template are recorded as part of the seat's agent identity (H-6). A silently updated model otherwise invalidates every prior benchmark without any signal that it happened.

**Capture rationales.** When B-2 produces reasoning tokens, store them in a side stream keyed to the event sequence — the same pattern as chat (P-8), and directly useful later for distillation into a trained agent.

### 9.5 Cost control (B-3)

Internal and flagged accounts only for now. Before any wider exposure: per-game and global spend caps, and graceful degradation to the heuristic bot when a cap is hit or the provider errors. Guests must never be able to initiate LLM spend.

### 9.6 Risks

1. **Prompt injection through chat.** If free-text chat is ever placed into an LLM player's prompt, players can issue instructions to the bot — "give me all your wood" is a trade negotiation to a human and an instruction to a model. **Do not include chat text in the LLM player's prompt.** If social play later demands it, isolate it as clearly-delimited untrusted data and never as instructions. This is the sharpest interaction between §8.5 and this section.
2. **Latency shapes the game feel.** An LLM call per decision at human pace is tolerable; hundreds per game is not. Forced-move auto-play is the primary mitigation, and turn timers need to accommodate bot think time.
3. **Model drift breaks benchmarks** — see the pinning requirement above.
4. **Cost per game is unknown** until measured. Measure it on internal games before B-3 is relaxed.
5. **Bootstrap data quality.** B-4 uses LLM games as training data; a systematically weak LLM teaches those weaknesses. Validate against the heuristic baseline before using its games to warm-start anything.

---

## 10. Analytics and player rating

Everything here is derived from the canonical event log (§7) and is regenerable. Nothing in this section is computed inside `catan-core`.

**Status:** the rating design (§10.5) is **decided** — register `A-1`…`A-4`. The statistical methods in §10.1–§10.4 and §10.6 are **recommended practice** rather than ratified decisions: they describe how to compute things correctly, not choices between valid alternatives.

### 10.1 Dice fairness — two different questions

These get conflated constantly, and they need opposite statistical treatment.

**(a) Was this game's dice sequence unusual?** Small sample — roughly 60–100 rolls. The theoretical distribution over 2–12 is known exactly: `(1,2,3,4,5,6,5,4,3,2,1)/36`.

- A plain chi-squared goodness-of-fit test is **not valid at this sample size**. With ~70 rolls the expected count for 2 and for 12 is under 2, well below the ≥5 rule of thumb. Either bin the tails — which discards exactly the information players care about — or compute an **exact Monte Carlo p-value** by simulating multinomial draws under the null. Simulation is cheap and correct; use it.
- Report an **effect size**, not just a p-value: KL divergence of the empirical distribution from theoretical, in bits. It is interpretable and comparable across games of different length.
- **Do not show a per-game p-value as a fairness verdict.** Across thousands of games, ~5% will clear p<0.05 by construction, and those are precisely the games players screenshot as proof of rigging. Present it as an **empirical percentile instead**: "the dice in this game deviated more than 87% of recorded games." Same information, no significance claim, no multiple-comparisons trap.
- If per-game p-values are ever used analytically across the corpus, apply Benjamini–Hochberg FDR control.

**(b) Is our RNG actually fair?** A trust and QA question, answered on the **pooled corpus** of millions of rolls, never per game.

- At that sample size chi-squared is valid — but the opposite problem appears: any trivial deviation becomes "significant". Report effect size (KL, maximum per-face deviation) alongside the p-value and judge on effect size.
- Marginal frequencies are not enough; a bad RNG can produce correct marginals with serial structure. Add an **independence check** — lag-1 autocorrelation and a Wald–Wolfowitz runs test over the pooled roll sequence.
- Track the 7 separately: it is 16.7% of rolls and drives the entire robber economy.

The general principle, worth stating once: **small n makes p-values invalid, large n makes them uninformative.** Both regimes need effect sizes.

### 10.2 Expected vs actual production

The flagship analysis, and the one that separates luck from play.

**Expected production per roll.** For player *i*, given the current board:

```
EPR_i = Σ  P(n_hex) × yield(building)        yield = 1 settlement, 2 city
      hexes adjacent to i's buildings         P(n) = pips(n) / 36
```

**Exact variance, no simulation needed.** Production on a single roll is a deterministic function of the roll, so per-roll production `X_i,t` has a known pmf over 11 outcomes. Rolls are independent, so over a game:

```
E[total_i] = Σ_t E[X_i,t]        Var[total_i] = Σ_t Var[X_i,t]
```

This holds even though buildings change during the game — each turn simply contributes its own term. A per-player, per-resource **z-score** `(actual − expected) / sd` follows directly and is exact.

**Decompose the gap.** A raw expected-vs-actual difference silently mixes four causes. Replay lets us separate all of them:

| Term | Meaning |
|---|---|
| `E_raw` | Expected production ignoring robber and supply limits |
| `RobberCost` = `E_raw − E_robber` | Expected production lost to the robber sitting on your hexes (R-5.8) |
| `SupplyDenial` | Production owed but not paid because a stack was empty (R-5.6) |
| `DiceLuck` = `A_ideal − E_robber` | What the dice actually did, given the real robber positions |

giving the identity:

```
Actual = E_raw − RobberCost − SupplyDenial + DiceLuck
```

This matters because the four have completely different meanings. `DiceLuck` is chance. `RobberCost` is *other players choosing to target you* — a social outcome, not a random one. `SupplyDenial` is a rules artefact. Reporting them as one number tells a player nothing about which happened.

**Presentation:** cumulative expected vs actual over turns, per player and per resource type, with the decomposition as a stacked breakdown. Per-resource z-scores answer "was I starved of ore specifically."

### 10.3 Descriptive statistics

**Per game**
Length in turns and wall time · winner and final VP breakdown · roll histogram with §10.1(a) percentile · 7-count · total production by resource · robber moves and target hexes · steal matrix (who robbed whom) · trade counts by type (player, 4:1, 3:1, 2:1) · offers made/accepted/rejected · development cards bought and played by type · VP progression curve per player · Longest Route and Largest Army holders over time and number of transfers · discards forced by 7s.

**Per player, within a game**
Expected vs actual production with the §10.2 decomposition · income by source (production, trade, Invention, Monopoly, steals) · outflow by sink (builds, trades, discards, robbed, Monopoly losses) · resources spent per VP earned · average and peak hand size · cards lost to discards · opening placement quality (pip count, resource diversity, port access) · trade profile (proposal rate, acceptance rate as proposer and as accepter, net resource balance per counterparty) · robber exposure (times targeted, cards lost) · think time by decision type.

**Per player, across games** *(requires H-6 identity)*
Games played, win rate, average finishing VP · win rate segmented by seat position, player count, setup variant, and rules version · opening preferences · build-order tendency (city-first vs expansion vs development cards) · trade behaviour and generosity · **luck-adjusted performance** (see §10.4) · rating and rating trajectory (§10.5).

**Corpus and balance**
Seat/turn-order win rate — the first-player advantage question · board layout imbalance · Fixed vs Variable setup outcomes · effect of the red-number option (R-3.12) · effect of trade mode · human vs bot and bot-version comparisons · the §10.1(b) RNG audit.

### 10.4 Luck-adjusted performance

Rating (§10.5) measures results; results in Catan carry a large chance component. The complementary metric: **VP earned relative to what the player's production entitled them to.**

Concretely — regress final VP on total production (or on the §10.2 z-scores) across the corpus, and report each player's **residual**. A player who consistently finishes above the curve converted resources better than average; one below did not. This is the single most useful "were you good or lucky" number, and it is only computable because §10.2 gives an exact expectation rather than an estimate.

### 10.5 Player rating

**"Halo ranking algorithm" is TrueSkill** — Microsoft Research, developed for Xbox Live and first deployed on Halo 2. It is a good instinct for this problem, for a specific reason: **Elo is fundamentally a two-player system**, and Catan is a 3–4 player free-for-all. Elo extensions to multiplayer are pairwise-decomposition hacks. TrueSkill models N-player outcomes natively and maintains a Gaussian belief `(μ, σ)` per player rather than a point estimate.

#### Decisions

| # | Question | Decision |
|---|---|---|
| A-1 | Model and implementation | **OpenSkill (Weng–Lin), Plackett–Luce variant.** TrueSkill-family behaviour, maintained open implementations, no patent exposure |
| A-2 | Pool segmentation | **One pool per major configuration** — (trade mode × major rules version) |
| A-3 | Guest rating | **Rated provisionally**, high σ, shown as provisional, carried across on account claim |
| A-4 | Seat position | **Randomise seating** and report per-seat win rates across the corpus |

*TrueSkill 2 (2018) adds margin and experience effects but has no public reference implementation. The patent position on TrueSkill proper is worth a legal check if A-1 is ever revisited — that is a flag, not advice.*

#### Design points

1. **Use the full finishing order, not just the winner.** Final VP totals rank all 3–4 players, so every game yields a complete ranking rather than one bit. Plackett–Luce consumes this natively (A-1), and it roughly triples the information per game — which matters given how slowly high-variance games converge.
2. **Display a conservative rating**, `μ − 3σ`, so new players aren't shown an inflated number before their uncertainty collapses. This is also what makes A-3's provisional guest ratings honest rather than misleading.
3. **Bots share the rating pool.** This is the baselining answer: a pinned heuristic bot with tight σ after thousands of games becomes an **absolute yardstick**. "Trained agent v4 at μ=32 vs heuristic at μ=25" is directly meaningful, and human ratings become comparable to bot ratings on one scale.
4. **Keep the set of "major" configurations deliberately small** (A-2). Every additional pool fragments player ratings and slows convergence, so a config should only earn its own pool when it genuinely changes how the game is played — trade mode does, a cosmetic option does not.
5. **Randomised seating (A-4) makes seat effects average out** over a player's games without modelling anything. It does *not* protect a player with very few games, so seat effects remain a known limitation at low game counts — visible in the per-seat corpus statistics rather than corrected in the rating.
6. **Exclude substituted games** (P-2) from rated updates — neither the departed human nor the bot that finished for them played a whole game.
7. **Guest ratings transfer on claim** (A-3) through the identity alias in §8.2 — never by rewriting historical games.
8. **Expect slow convergence.** Catan's variance means σ shrinks slowly; show it rather than hiding it, and resist ranking players publicly before σ is small.

#### Known exposure

A-3 rates guests, and guest identity is a device-persistent ID (P-1). That combination is **smurf-friendly**: a player wanting a fresh rating can clear device state. Accepted for now — the alternative costs real signal from every guest game — but if rated play ever carries stakes (leaderboards, rewards, competitive matchmaking), revisit A-3 rather than trying to patch it downstream.

### 10.6 Statistical pitfalls to design around

1. **Multiple comparisons** on per-game dice tests — §10.1.
2. **Truncation bias.** Games end when someone reaches 10 VP, so "average VP at turn 25" includes only games that lasted 25 turns, biasing toward slow games. Report n per turn explicitly, or use survival-analysis framing.
3. **Players within a game are not independent.** One player's gain is literally another's loss; treating player-games as i.i.d. samples will understate variance in any aggregate.
4. **Configuration heterogeneity.** `rules_version`, trade mode, setup variant, and the R-3.12 option all change gameplay — mandatory filter columns, per §7.4.
5. **Bot games can swamp human data.** Self-play corpora are orders of magnitude larger; never pool them with human games without explicit segmentation.
6. **Survivor bias in player stats.** Players who quit early are underrepresented in long-run aggregates.

---

## 11. Content assets still to transcribe *(deferred — blocks Fixed Setup)*

These exist only as artwork and are **not** transcribed in this document, per the current scoping decision. Someone with the physical components or a high-resolution scan needs to fill them in:

| ID | Asset | Needed for |
|---|---|---|
| ART-1 | Exact **Fixed Setup** layout: terrain hex + number disc for each of the 19 board positions (p.4–5 diagram) | Fixed Setup only |
| ART-2 | Exact **Fixed Setup** starting positions of the 8 settlements and 8 roads, and which settlement is each player's "second" (p.5 diagram) | Fixed Setup only |
| ART-3 | **Port layout** per sea frame piece: port type, its two intersections, and each piece's puzzle-end numbers (p.3 artwork) | Both setups |
| ART-4 | **A–R letter → number mapping** on the number disc backs (p.3, p.11) | Variable Setup |

ART-3 and ART-4 are needed for *any* implementation; ART-1 and ART-2 only gate the Fixed Setup mode. The Variable Setup is fully specified in rules text (R-3), so a first implementation can ship with Variable Setup alone.

---

## 12. Resolved rulings (edge cases)

The CN3081 rulebook leaves the following open. Each is now resolved against the **official CATAN FAQ** or the **5th Edition Almanac** ([§14](#14-sources)). These are marked `FAQ`/`ALM` in the rule tables above, and are clarifications rather than rulebook text.

| # | Question | Official ruling | Source |
|---|---|---|---|
| 1 | Robber moved to a hex with no buildings? | Legal. The robber must move to a different hex; if no opponent has a building there, no card is stolen. Production on that hex is still blocked. | FAQ, ALM |
| 2 | Robbing a player who has no resource cards? | "Bad luck" — no card is drawn, and the active player does not get to pick a different victim. | FAQ |
| 3 | Must the robber move at all? | Yes. It must be placed on a *different* hex; the desert is a legal destination. | FAQ, ALM |
| 4 | Can the robber block building or port trading? | No. It only blocks production on its own hex. | FAQ |
| 5 | Bonus tile ties? | The tile stays with its current owner. Transfer requires strictly more. | FAQ |
| 6 | Longest Route broken — who gets it? | Current holder keeps it if they still qualify; another player takes it if they now qualify; **no one** holds it if zero or multiple players tie for longest. It may be regained via a bypass. | FAQ |
| 7 | Do forks count toward route length? | No. A route is a single continuous path between two intersections; own buildings don't break it, opponents' do. | FAQ, ALM |
| 8 | Can Largest Army be lost? | Only to a player with strictly more *played* Knights. Knights never leave the table, so the tile never returns to the unowned pool once claimed. Unplayed Knights count for nothing. | FAQ |
| 9 | Monopoly — must players reveal hands? | No. The rules assume honesty; players must answer truthfully about their holdings but need not show cards. | FAQ |
| 10 | Trading after a 7 is rolled? | Not allowed until the discard and robber resolution are complete; the turn then continues normally. | FAQ |
| 11 | Knight before the roll, then a 7? | Both resolve — the robber is activated twice. Playing a development card is independent of the dice result. | FAQ |
| 12 | Can a player voluntarily discard or gift cards to dodge the robber? | No. No voluntary reduction, no gifts, no one-sided trades, no credit. | FAQ |
| 13 | Three-way / secret trades? | Both forbidden. Every trade is public and strictly two-party, with the active player as one party. | FAQ |
| 14 | Use an opponent's port? | No. Only the owner of a building on a port may use it. | FAQ |
| 15 | How many development cards per turn? | Buy as many as you can pay for; play at most one (VP cards excepted). | FAQ |
| 16 | Can a player win outside their own turn? | No. Victory is checked and claimed only on the winner's own turn — but it is then immediate and irrevocable. | FAQ |
| 17 | Settlement placement without a road connection? | Only during setup. Afterwards every settlement needs an own-road connection. | FAQ |
| 18 | City on an empty intersection? | Never — cities only upgrade an existing own settlement. | FAQ |
| 19 | Rebuild on an intersection freed by a city upgrade? | Yes, the returned settlement is reusable; buildings may never be *moved*, though. | FAQ |
| 20 | Coastal intersections without a port? | Legal building spots. Any point where three hexes meet is an intersection. | FAQ |

### 12.1 Residual unknowns

Questions not settled by any official source. All six have now been **decided for this project** — these are house rulings, not CATAN rules, and are marked `HOUSE` where they appear in the rule tables.

| # | Question | Decision | Rule |
|---|---|---|---|
| D-1 | Road Building with fewer than 2 legal placements (blocked board or short road pool) | **Place as many as are legal** (1 or 0). The card is discarded and the turn's development-card allowance is consumed. | R-9.10a |
| D-2 | Discard resolution order on a 7 | **Simultaneous.** All affected players discard at once; play resumes when the last confirms. | R-6.2a |
| D-3 | Taking a resource whose supply stack is empty (4:1/port trade, or Invention) | **Must pay in full.** The trade is illegal unless the stack can supply the whole amount; Invention takes as many as remain (possibly 1 or 0). | R-7.17 |
| D-4 | Same resource type on both sides of a multi-type player trade | **Forbid any overlap.** No resource type may appear on both sides of a trade. | R-7.18 |
| D-5 | Trade offer lifecycle | **Open market** — see D-5 notes below. | R-7.19 |
| D-6 | Red numbers (6/8) adjacent on a randomly generated board | **Game option**, defaulting to the constraint enabled. | R-3.12 |

**D-5 notes — open market.** Multiple offers stay live at once and any player may accept any live offer at any point during the active player's turn. This is the most table-like option and the most state to manage; three consequences follow:

- Every live offer must still have the **active player as one party** (R-7.3). Offers between two non-active players are never valid, even in an open market.
- **Acceptance races are real.** Two players may accept offers the proposer can only cover once. Resolve atomically on a first-come basis, re-validating both parties' holdings at the moment of execution, and reject the loser with a clear reason rather than silently dropping it.
- Offers must be **re-validated, not merely displayed**, when the board or a hand changes. An offer that was legal when made can become illegal (cards spent on a build, a Monopoly played); it should be invalidated rather than executed against stale state.

**Still open — verification tasks** (not decisions; they need the physical components or a second look at the source):

1. **Building costs** are read from player-aid iconography (§2.8), not from rules prose — verify against the physical aid.
2. **Port count and distribution** (4× 3:1, 5× 2:1) are read from component artwork; the 5th Edition's 9 harbour pieces corroborate the total but not the 6th Edition's fixed distribution across the 6 frame pieces.
3. **Rulebook completeness.** This extraction covers a 12-page rulebook; confirm no supplementary material (e.g., a separate almanac insert) belongs in scope.
4. **The four inferred trade rules** — R-7.13, R-7.14, R-7.15, R-7.10 — rest on reading rather than source text. R-7.15 (no trading before the dice roll) is the least certain, since a development card *may* legally be played pre-roll.

### 12.2 Edition drift when consulting official sources

The FAQ and Almanac are written for the 5th Edition and use older terms. Translation table, to avoid mis-citing them:

| 6th Ed. (CN3081) | 5th Ed. / FAQ |
|---|---|
| Longest Route | Longest Road |
| Supply | Bank |
| Invention | Year of Plenty |
| Development card colours (single back) | Progress cards (green frame) / Knight cards (purple frame) |
| Ports (printed on frame) | Harbours (9 loose harbour pieces, randomly placed) |
| Sea frame pieces (6, puzzle-jointed) | Frame pieces (fixed layout) |
| 3-player game removes **white** | 3-player game removes **red** |
| First player = highest dice roll | Starting setup: oldest player |
| Number disc | Number token |
| Single Action phase, any order (R-7.1) | Separate trade phase / build phase, with a "combined phase" variant recommended for experienced players |
| 4:1 trade takes a **different** resource | 4:1 trade takes "**any** 1 resource card of your choice" |

Two of these are **rule changes, not just renames**, and the FAQ cannot be used to validate the 6th Edition behaviour:

- The 6th Edition requires the resource taken in *any* maritime trade to differ from the one given (R-7.6–R-7.8); the 5th Edition permitted taking any type. Practically harmless — trading 4 wood for 1 wood is self-defeating — but a validator written from the FAQ would be wrong.
- The 6th Edition has no trade/build phase separation at all; trading and building interleave freely (R-7.1). Any FAQ answer conditioned on "strict separation" is inapplicable.

The 5th Edition also adds a Variable-Setup constraint the 6th Edition rulebook does not state: **red numbers (6 and 8) must not be adjacent** in a fully random layout. Decide whether to adopt it — it materially affects generated-board quality and is a common expectation.

### 12.3 Trade rules — completeness audit

Trade is the least completely specified area of the rulebook. Status of every trade question identified:

| Question | Status | Rule |
|---|---|---|
| Who may trade with whom on a turn | **Defined** | R-7.3 |
| Three-way / secret / credit trades | **Defined** (all forbidden) | R-7.3, R-7.5, R-7.11 |
| No gifting; no matching-type trades | **Defined** | R-7.5 |
| 4:1 / 3:1 / 2:1 ratios and port access | **Defined** | R-7.6–R-7.9 |
| Taken resource must differ from given | **Defined** | R-7.6–R-7.8 |
| Using an opponent's port | **Defined** (forbidden) | R-7.9 |
| Development cards tradeable | **Defined** (no) | R-7.10 |
| Robber blocking trade | **Defined** (no) | R-7.12 |
| Repeat trades within one turn | **Defined** | R-7.1, R-7.16 |
| Counteroffers and player-initiated proposals | **Defined** | R-7.4 |
| Trade after a 7, before robber resolution | **Defined** (forbidden) | R-6.6 |
| Player-trade ratios / multi-type trades | **Inferred** — implied by "resource(s)", never stated | R-7.13 |
| Obligation to accept; when an offer binds | **Inferred** | R-7.14 |
| Trading before the dice roll | **Inferred** — placement in the Action phase implies no, but never stated, and a *development card* may be played pre-roll, so the phase boundary is not simply "nothing before the roll" | R-7.15 |
| Bonus tiles tradeable | **Inferred** (no) — never mentioned in any source | R-7.10 |
| Trading from an exhausted supply stack | **House ruling** (D-3) | R-7.17 |
| Same type on both sides of a multi-type trade | **House ruling** (D-4) | R-7.18 |
| Offer lifecycle / binding / concurrency | **House ruling** (D-5) | R-7.19 |

Twelve trade questions are settled by the rulebook or FAQ, four rest on inference that should still be confirmed (R-7.10, R-7.13, R-7.14, R-7.15), and three had no answer in any source — those are now house rulings, marked as such so they are never mistaken for CATAN rules.

The open-market decision (D-5) makes trade the most concurrency-sensitive part of the system: it is the only place where a non-active player initiates a state change, and the only place where two valid requests can race. Treat `ACCEPT` as a transaction against current state, never against the state the offer was authored in.

---

## 13. Next steps

**Unblock (no dependencies, needed by everything)**

1. Transcribe artwork assets **ART-3 and ART-4** (§11) — port layout and disc letters gate every mode. ART-1/ART-2 gate Fixed Setup only.
2. Close the four verification tasks in §12.1 — building costs, port distribution, rulebook completeness, and the four inferred trade rules.

**Engine**

3. Board topology module (19/54/72) with precomputed adjacency, validated against the §5.5 invariants.
4. `catan-core` (§6.2): bitboard state, enum state machine, legal-move generation, plus a benchmark harness to replace the §6.6 *targets* with measurements.
5. Every `R-x.y` rule becomes an acceptance test; §5.7 is the priority set. The six `HOUSE` rules (R-3.12, R-6.2a, R-7.17, R-7.18, R-7.19, R-9.10a) need tests most — they have no external source to fall back on.
6. Variable Setup first (fully specified in text); Fixed Setup once ART-1/ART-2 exist.
7. Build the **trade mode seam** (`full` / `restricted` / `disabled`, §6.5) early — RL (§6.5) and the LLM player (§9.4) both depend on it, independently.

**History and data**

8. Event log and replay path (§7.2, §7.3) built *alongside* the engine, not after — retrofitting event emission into a finished engine is far more invasive than emitting from the start.
9. Redaction leak test (§7.6 risk 2) and snapshot-checksum verification (§7.6 risk 3) in the first replay milestone. P-6 fogged spectating makes the leak test a **launch blocker**, not hardening.
10. Own principal table and the guest-claim alias design (§8.2) before any account exists — retrofitting identity onto immutable logs is not possible.
11. Human-identity retention and deletion policy (§7.6 risk 5) before the first human game is recorded.

**Platform**

12. Auth (P-4), lobby lifecycle and config (§8.3), turn timers, and seat-actor substitution events (§8.2).
13. Text chat as a separate stream (§8.5) with mute, block, and report from the start — moderation is much harder to add to a live product than to build into it.

**Bots**

14. Heuristic bot — a **launch prerequisite**, since P-2 disconnect takeover, lobby filling, and LLM fallback all depend on it.
15. Forced-move auto-play in the engine (§9.4) — it cuts LLM call volume and RL rollout cost simultaneously.
16. LLM player behind the B-3 flag, with pinned model version and spend caps; measure real cost per game before considering wider access.

**Analytics**

17. Expected-production engine (§10.2) — the exact mean/variance computation and the four-way decomposition. It underpins per-game luck reporting *and* the luck-adjusted metric in §10.4, so build it once, in the replay layer.
18. Dice reporting as an **empirical percentile**, not a per-game p-value (§10.1a), with the pooled RNG audit (§10.1b) as a separate scheduled job.
19. Implement rating (A-1…A-4, §10.5) as a post-game batch job over completed, non-substituted games, with seat assignment randomised at lobby start.
20. Encode the §10.6 pitfalls as constraints in the analytics layer — mandatory config filters, explicit per-turn n, no i.i.d. pooling of player-games.

**Decision register:** six rules decisions (§12.1), seven data decisions (§7.1), eight platform decisions (§8.1), four bot decisions (§9.1), four rating decisions (§10.5) — 29 in total. Nothing blocks starting the engine except the ART-3/ART-4 artwork data.

---

## 14. Sources

1. *CATAN — The Game* rulebook, CN3081, v6.250401, 6th Edition, © 2025 CATAN GmbH / CATAN Studio — the primary source, supplied as PDF.
2. [Official CATAN base-game FAQ](https://www.catan.com/faq/basegame) — used for all `FAQ`-marked rulings.
3. [CATAN 5th Edition Game Rules & Almanac (PDF)](https://www.catan.com/sites/default/files/2021-06/catan_base_rules_2020_200707.pdf) — used for all `ALM`-marked rulings.

Where the 6th Edition rulebook and the older sources conflict, **the CN3081 rulebook wins**; the FAQ and Almanac are used only to fill gaps it leaves silent.
