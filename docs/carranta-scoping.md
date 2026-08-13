# Carranta — Rules & Materials Scoping Document

**Status:** Draft 9 — rules, engine architecture, history/data model, platform scope, bot strategy, analytics/rating, client and operational choices recorded. 46 decisions registered.
**Target:** **Digital implementation** of Carranta: a Rust engine core (§6) usable both as a game service and as a high-throughput environment for AI training, with full game history capture (§7), a lobby-based multiplayer platform (§8), heuristic and LLM opponents (§9), and a statistics and rating layer (§10). This document is the reference for the state schema, action model, rules validation, data model, product scope, and analytics methods.

**What Carranta is**

A hex-tile resource-trading and settlement-building game for 3–4 players — an original implementation of a well-established board game genre. Mechanics and systems are not protectable by copyright; specific rulebook wording, artwork, names, and trademarks are. This document deliberately contains none of the latter: the rule set below is Carranta's own specification, written in its own words, and no element name, board layout, or text is carried over from any published product. *This is a statement of intent and of how the document was written, not a legal opinion — worth a lawyer's review before publication.*

**Scope boundary**

- **In scope:** the core game, **3–4 players**, both setup modes (Beginner and Random).
- **Out of scope:** larger player counts, team play, seafaring or campaign variants, and any expansion mechanics. The state model should not hard-code a player count of 4, but no expansion is specified here.

**Conventions used here**

- Rules are numbered `R-x.y` so they can be referenced from tickets, tests, and code.
- Where a rule embodies a deliberate design choice rather than an obvious consequence of the system, it cites the decision that settled it (`D-n`, §12.1).
- Content marked *(to author)* is a design asset that does not exist yet — see §11.

---

## 1. Game parameters

| Parameter | Value |
|---|---|
| Players | 3–4 (in a 3-player game the white pieces are not used) |
| Win condition | First player to reach 10 VPs **on their own turn** |
| Turn order | Clockwise, starting with the first player |
| Turn structure | Production phase → Action phase |
| Setup modes | Beginner Setup (prescribed layout) / Random Setup |
| Board topology | 19 hexes, 54 intersections, 72 edges |

---

## 2. Rules

### 2.1 Objective (R-1)

| ID | Rule |
|---|---|
| R-1.1 | The first player to reach 10 victory points (VPs) on their turn wins. |
| R-1.2 | VPs are earned by building. Resources needed for building are collected and traded for. |
| R-1.3 | VP sources: settlement = 1 VP, city = 2 VPs, Longest Road tile = 2 VPs, Largest Militia tile = 2 VPs, each Victory Point development card = 1 VP, road = 0 VP. |

### 2.2 Setup — Beginner Setup (R-2)

| ID | Rule |
|---|---|
| R-2.1 | **Assemble the frame.** Match the numbers at the puzzle-piece ends of the 6 sea frame pieces to assemble the coast of Carranta. |
| R-2.2 | **Place hexes and number discs** inside the frame exactly as shown in the setup diagram (fixed, prescribed layout). |
| R-2.3 | **Create the supply.** Sort resource cards by type into five **faceup** stacks in the card trays. Shuffle the development cards into one **facedown** stack in the remaining card tray slot. Place the Longest Road and Largest Militia tiles near the board. |
| R-2.4 | **Place the robber** on the desert hex. |
| R-2.5 | Each player selects a color and takes that color's roads and buildings (settlements + cities) plus a player aid. In a 3-player game the white pieces are not used. |
| R-2.6 | Place 2 starting settlements and 2 roads per player, at the prescribed positions shown in the diagram. |
| R-2.7 | **Starting resources.** Each player takes from the supply the resource cards matching the hexes adjacent to their **second** settlement (highlighted in the diagram). These cards are kept **hidden in hand**. |
| R-2.8 | **First player.** Each player rolls the dice; highest roll is the first player. |

### 2.3 Setup — Random Setup (R-3)

| ID | Rule |
|---|---|
| R-3.1 | **Assemble the frame.** Shuffle the sea frame pieces and connect their puzzle-piece ends (random coast, port positions vary). |
| R-3.2 | **Place the hexes** randomly, face up, inside the frame. |
| R-3.3 | **Place the number discs.** Arrange the discs facedown in A-B-C (alphabetical) order. Starting at any corner of the board, place them on the hexes counterclockwise, **skipping the desert**. Then flip them so the number side is faceup. |
| R-3.4 | **Place the robber** on the desert hex. |
| R-3.5 | **Create the supply** — identical to R-2.3. |
| R-3.6 | **First player.** Each player rolls the dice; highest roll is first player. Then each player selects a color and takes their roads, buildings, and a player aid. |
| R-3.7 | **Placement round 1.** The first player places 1 settlement on an empty intersection of their choice, then 1 road on an empty edge adjacent to that settlement. Continue **clockwise** (to the left) until every player has 1 settlement and 1 road. |
| R-3.8 | **Placement round 2.** Starting with the last player and going in **reverse order**, each player places 1 settlement on an empty intersection of their choice and their second road on an empty adjacent edge. |
| R-3.9 | The Distance Rule applies to all setup settlement placements: stay at least two edges away from all other settlements. |
| R-3.10 | **Starting resources.** Each player takes 1 matching resource card from the supply for each hex adjacent to their **second** settlement. Kept **hidden in hand**. |
| R-3.11 | Setup placement is the **only** time a settlement may be placed without connecting to one of your own roads. |
| R-3.12 | **Red-number adjacency** (6 and 8) on a randomly generated board is controlled by a game option, **enabled by default**: when on, generated boards where two red numbers are adjacent are rejected or repaired. Without it, a random layout can concentrate the two highest-probability numbers on adjacent hexes. |

### 2.4 Turn structure (R-4)

| ID | Rule |
|---|---|
| R-4.1 | Play proceeds in turns, starting with the first player, clockwise around the table. |
| R-4.2 | A turn consists of exactly two phases in order: 1. Production phase, 2. Action phase. |
| R-4.3 | After finishing the Action phase, if the player has not won, they pass the dice to the player on their left, who begins their Production phase. |

### 2.5 Production phase (R-5)

| ID | Rule |
|---|---|
| R-5.1 | **(Optional) Play a development card** before rolling the dice. |
| R-5.2 | **Roll dice.** Roll both dice and add them. The total determines which hexes produce this turn. |
| R-5.3 | **Production.** Every hex whose number disc matches the roll produces. Each player with a **settlement** on a producing hex receives 1 resource card of that hex's type from the supply. |
| R-5.4 | A player with 2 or 3 settlements on the same producing hex receives 1 card per settlement. |
| R-5.5 | A player receives **2** resource cards for each of their **cities** on a producing hex. |
| R-5.6 | **Supply shortage.** If there are not enough cards of a produced resource in the supply to satisfy everyone's production, **no one** receives any of that resource. Exception: if only **one** player is affected, that player receives as many of those cards as remain in the supply. |
| R-5.7 | Terrain → resource mapping: forest → wood, hills → brick, pasture → wool, mountains → ore, fields → wheat, desert → nothing. |
| R-5.8 | A hex occupied by the robber does **not** produce resources when its number is rolled. Other hexes of the same type — including ones bearing the same number — still produce normally. |

### 2.6 Rolling a 7 (R-6)

| ID | Rule |
|---|---|
| R-6.1 | On a roll of 7, **no hex produces** any resources. |
| R-6.2 | **Discard.** Each player (all players, not just the active one) holding **more than 7** resource cards must choose half of them, rounded down, and return them to the supply. *(Example: 9 cards in hand → discard 4.)* |
| R-6.2a | Discards resolve **simultaneously** — all affected players choose at once, and play continues when the last confirms. |
| R-6.3 | **Activate the robber.** The active player **must** move the robber to a **different** hex — leaving it in place is not a legal option. Any other land hex is a legal destination, including the desert. |
| R-6.4 | The active player then steals 1 **random** resource card from an **opponent** who has a building on the robber's new hex. If several opponents have buildings there, the active player chooses which one to rob. The victim holds their cards facedown; the card is taken at random, unseen. |
| R-6.5 | Development cards are never stolen by the robber, and do not count toward the discard threshold in R-6.2. |
| R-6.6 | No trading is allowed between the dice roll and the completion of the discard + robber resolution. The active player continues their turn normally afterwards. |

### 2.7 Action phase — Trade (R-7)

| ID | Rule |
|---|---|
| R-7.1 | Actions may be taken as often as desired and in any order, as long as the player has the resources. |
| R-7.2 | The active player may trade freely with other players and with the supply. |
| R-7.3 | During a player's turn, other players may only trade **with the active player** — not with each other and not with the supply. Triangular (three-way) trades are forbidden. |
| R-7.4 | **Player trade.** The active player announces which resource(s) they want and which they offer. Other players may accept, counteroffer, or make their own proposals. |
| R-7.5 | **No gifting.** Cards may not be given away in any form. This includes trading matching resource types (e.g., 3 ore for 1 ore is not allowed). A trade must involve giving *and* taking resources — nothing may be traded for nothing, for a service, or for a promise (no credit or deferred trades). |
| R-7.6 | **General supply trade (4:1).** Put 4 identical resource cards into the supply and take 1 card of a **different** resource. |
| R-7.7 | **3:1 port trade.** With a building on a 3:1 port, put 3 identical resource cards into the supply and take 1 card of a **different** resource. |
| R-7.8 | **2:1 port trade.** With a building on a 2:1 port, put 2 cards of the resource shown on that port into the supply and take 1 card of a **different** resource. |
| R-7.9 | Port access requires the player's own building (settlement or city) on that port's intersection. A player may never use an opponent's port. |
| R-7.10 | Development cards may not be traded or given away. Bonus tiles are likewise never transferable by trade — they move only by meeting their own conditions (R-10). |
| R-7.11 | Trades must be public: no secret trades, and no player may be required to reveal their hand. |
| R-7.12 | The robber never blocks trading, including port trades. |
| R-7.13 | **Player trades have no ratio restriction.** Any number of cards and any mix of types may be exchanged for any other, subject only to R-7.5. There is no requirement that the counts match or that either side be a single type. |
| R-7.14 | No player is ever obliged to trade or to accept an offer, and the active player is not bound by an announced offer until the exchange is made. |
| R-7.15 | Trading is confined to the Action phase. No trade may occur before the dice roll, nor during discard/robber resolution (R-6.6). |
| R-7.16 | Maritime trades (4:1, 3:1, 2:1) may be performed as often as the player can pay for them, and are always available regardless of ports — only the improved ratios require a port. |
| R-7.17 | **Empty supply stack.** A maritime trade is illegal unless the target stack can supply the full amount taken. An Invention card takes as many of the requested cards as remain (possibly 1 or 0). |
| R-7.18 | **No type overlap.** No resource type may appear on both the give and take side of a single trade — a generalization of R-7.5 covering multi-type offers. |
| R-7.19 | **Open-market offers.** Multiple trade offers may be live simultaneously, and any player may accept any live offer during the active player's turn. Every offer must still have the active player as one party (R-7.3). Acceptances resolve atomically, first-come, re-validating both parties' holdings at execution; offers invalidated by an intervening state change are rejected with a reason, never executed against stale state. |
| R-7.20 | **Offer limits** (D-7). A player may make at most ~20 trade offers per turn, with a short minimum interval between them. Enforced in the engine, not the client — client-side limits are advisory, and the threat model is a client speaking the protocol directly. Rejections are surfaced to the offender, never silent. |

### 2.8 Action phase — Build (R-8)

Building costs — a working set, not yet balance-tested (§12.1):

| Structure | Cost | VP |
|---|---|---|
| Road | 1 brick + 1 wood | 0 |
| Settlement | 1 brick + 1 wood + 1 wool + 1 wheat | 1 |
| City | 2 wheat + 3 ore | 2 |
| Development card | 1 wool + 1 wheat + 1 ore | ? (0 or 1) |

| ID | Rule |
|---|---|
| R-8.1 | To build, return the required resource cards from hand to the supply. |
| R-8.2 | **Roads** are placed on empty hex edges — one road per edge. A new road must connect to one of the player's own existing roads, settlements, or cities. Coastal edges are legal. |
| R-8.3 | A road may not be built starting on the far side of an **opponent's building** — an opponent's settlement or city blocks continuation through that intersection. |
| R-8.4 | **Settlements** are placed on empty intersections, must satisfy the Distance Rule, and must connect to at least one of the player's own roads (after setup). Any point where three hexes meet is a legal intersection, including coastal points without a port. |
| R-8.5 | **Distance Rule.** When placing a settlement, stay at least two edges away from all other buildings (own and opponents'). Equivalently: every building must remain surrounded by three unoccupied intersections, for the whole game. |
| R-8.6 | A player has 5 settlement pieces; to build further settlements, one must first be upgraded to a city. Piece pools are hard caps: 5 settlements, 4 cities, 15 roads. |
| R-8.7 | **Cities always replace settlements.** Remove one of your settlements from the board, return it to your player area, and place the city on that intersection. A city may never be built on an empty intersection. |
| R-8.8 | A player has 4 cities and may not build more. |
| R-8.9 | **Development cards** are bought by drawing the top card of the facedown deck. A player may buy as many per turn as they can pay for. |
| R-8.10 | If the development card deck runs out, no more development cards may be built. Development cards never return to the supply. |
| R-8.11 | Buildings may never be relocated. A settlement removed by a city upgrade returns to the player's pool and may be rebuilt later at a legal intersection. |
| R-8.12 | The robber does not block building. |
| R-8.13 | Intersections cannot be reserved — a player may not claim a spot without building on it. |

### 2.9 Development cards (R-9)

| ID | Rule |
|---|---|
| R-9.1 | Development cards stay **hidden** until played. |
| R-9.2 | Development cards do **not** count toward hand size when a 7 is rolled, and cannot be stolen by the robber. |
| R-9.3 | A player may play **at most 1** development card per turn, placing it **face up** in their player area. |
| R-9.4 | A development card may not be played on the turn it was bought. |
| R-9.5 | A development card may be played either **before rolling the dice** or at any time during the Action phase — including in the middle of trading. Playing one before the roll consumes the turn's single card play. |
| R-9.6 | Development cards may not be traded or given away, and never go back into the supply. |

**Card effects**

| ID | Card | Effect |
|---|---|---|
| R-9.7 | **Militia** (14×) | Activate the Robber (R-6.3, R-6.4): move the robber to a different hex and steal 1 random resource card from an opponent with a building on that hex. Played Militia remain face up for the rest of the game. |
| R-9.8 | **Invention** (2×) | Draw 2 resource cards of your choosing from the supply into hand. They may be the same type or two different types. |
| R-9.9 | **Monopoly** (2×) | Name a single resource type. Every opponent surrenders their entire holding of that type to you. Exactly one type may be named per play, however many or few cards that yields. Opponents answer truthfully but are not required to display their hands. |
| R-9.10 | **Road Building** (2×) | Place 2 roads without paying their cost. All ordinary placement restrictions still apply. |
| R-9.10a | — | If fewer than 2 legal road placements exist (blocked board or road pool short), place as many as are legal (1 or 0); the card is still discarded and the turn's development-card allowance is still consumed. |
| R-9.11 | **Victory Point** (5×) | Worth 1 VP. Must be kept **hidden** in the player area unless revealing them reaches the VP total needed to win; then reveal all VP cards at once, including those built this turn. |
| R-9.12 | **VP card exception.** Any number of VP cards may be played, even on the turn they were bought, in order to win — this bypasses R-9.3 and R-9.4. |

### 2.10 Bonus tiles (R-10)

| ID | Rule |
|---|---|
| R-10.1 | **Longest Road** (2 VPs). The first player with **5 continuous roads** in play receives the tile. |
| R-10.2 | If another player has **more** continuous roads in play, they immediately receive the tile. |
| R-10.3 | A route is a continuous path of road segments connecting two intersections, not interrupted by another player's pieces. **Forks do not add length** — only the longest single path counts. Your own settlements and cities do **not** interrupt your route; an opponent's building does. Closed loops are possible and count as their individual segments. |
| R-10.4 | A route can be **broken** by an opponent building a settlement on an intersection within it, splitting it into two segments. All normal building rules must be observed to do this. |
| R-10.5 | If a player's route is broken such that they no longer meet the requirement, the tile returns to the supply and stays there until a **single** player has the longest continuous route of at least 5 roads; that player immediately receives the tile and its 2 VPs. If the current holder still qualifies, they keep it. |
| R-10.6 | **Ties do not transfer.** A tile passes only to a player with strictly *more* than the current holder; on a tie the tile stays with its current owner. |
| R-10.7 | A broken route may be repaired by building a "bypass" — a detour around the blocking building. |
| R-10.8 | **Largest Militia** (2 VPs). The first player with **3 Militia cards in play** receives the tile. If another player has more Militia in play, they immediately receive it. Only *played* (faceup) Militia count; Militia in hand count for nothing. |

### 2.11 Winning (R-11)

| ID | Rule |
|---|---|
| R-11.1 | If a player has 10 or more VPs at any point **during their own turn**, the game ends immediately and they win. A player can never win during another player's turn. |
| R-11.2 | To claim victory, the player turns over any number of Victory Point cards, including ones built that turn, to demonstrate reaching 10 VPs. |
| R-11.3 | VP tally components: settlements (1 each), cities (2 each), Longest Road tile (2), Largest Militia tile (2), VP cards (1 each), roads (0), Militia played (0 in themselves). |
| R-11.4 | Victory is immediate and cannot be declined or deferred. If the winning player does not notice, the other players should tell them — a won game cannot be taken back. |

---

## 3. Materials — inventory

### 3.1 Board & terrain

| # | Item | Qty | Detail |
|---|---|---|---|
| M-01 | Sea frame pieces | 6 | Puzzle-piece ends with matching numbers; carry the ports |
| M-02 | Ports (printed on frame) | 9 *(to author — ART-1)* | 4× 3:1 generic; 5× 2:1 (one each: brick, wood, wool, wheat, ore) — a working default, not a balanced design |
| M-03 | Terrain hexes | 19 | 4× forest, 4× pasture, 4× fields, 3× hills, 3× mountains, 1× desert |
| M-04 | Number discs | 18 | 1×2, 2×3, 2×4, 2×5, 2×6, 2×8, 2×9, 2×10, 2×11, 1×12. 6 and 8 printed in red. Backs lettered A–R for the random setup |
| M-05 | Robber | 1 | Grey/neutral figure |

### 3.2 Cards

| # | Item | Qty | Detail |
|---|---|---|---|
| M-06 | Resource cards | 95 | 19× each of brick, wood, wool, wheat, ore. Shared card back |
| M-07 | Development cards | 25 | 14× Militia, 5× Victory Point, 2× Monopoly, 2× Road Building, 2× Invention. Shared card back |

### 3.3 Player pieces (4 colors: blue, red, white, orange)

| # | Item | Qty | Per player |
|---|---|---|---|
| M-08 | Settlements | 20 | 5 per color |
| M-09 | Cities | 16 | 4 per color |
| M-10 | Roads | 60 | 15 per color |

### 3.4 Tiles, accessories & derived items

| # | Item | Qty | Detail |
|---|---|---|---|
| M-11 | Longest Road tile | 1 | Bonus VP tile, 2 VPs |
| M-12 | Largest Militia tile | 1 | Bonus VP tile, 2 VPs |
| M-13 | Dice | 2 | Standard d6 (one red, one yellow) |
| M-14 | Player aids | 4 | Front: building costs; back: turn overview + development card rules |
| M-15 | Card trays | 2 | 6 slots total: 5 resource stacks + 1 development deck |
| M-16 | Rules reference | 1 | Not a play component |

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
| **Terrain hex** (M-03) | in frame slot | `PUBLIC` (faceup once placed) | Momentarily `HIDDEN` while being drawn randomly in random setup (R-3.2) |
| **Number disc** (M-04) | on a hex / off-board (desert has none) | `HIDDEN` (facedown, letter side up during random setup) → `PUBLIC` (number faceup) | Variable setup explicitly uses a facedown ordered phase (R-3.3) |
| **Robber** (M-05) | on exactly one hex (starts on desert) | `PUBLIC` | Always public; blocks production on its hex (R-5.8) but not building or trade (R-7.12, R-8.12) |
| **Resource card** (M-06) | supply stack / player hand / in-transit during trade / discarded to supply | supply: `PUBLIC` (faceup stacks, count visible) · hand: `OWNER` + `COUNTABLE` · trade: `TRANSIENT` (revealed to the trade partner and, being a public trade, to the table) · stolen: `TRANSIENT` to the thief only, never revealed to the table | Supply counts are public and may be checked before distribution (R-5.6). Steals are random and unseen (R-6.4). Monopoly forcibly reveals part of every hand for one resource type (R-9.9) |
| **Development card** (M-07) | facedown deck / player hand (unplayed) / player area (played, faceup) / VP card (held hidden until win) | deck: `HIDDEN` (order unknown to all; remaining count `PUBLIC`) · hand: `OWNER` + `COUNTABLE` · played: `PUBLIC` · VP card: `OWNER` until win, then `PUBLIC` | Never returns to supply (R-9.6). Played Militia stay faceup and are publicly counted for Largest Militia (R-10.8) |
| **Settlement** (M-08) | player pool (unbuilt) / on intersection / returned to pool on city upgrade | `PUBLIC` in all states | 5 per player, hard cap (R-8.6) |
| **City** (M-09) | player pool / on intersection | `PUBLIC` | 4 per player, hard cap (R-8.8); always replaces a settlement (R-8.7) |
| **Road** (M-10) | player pool / on edge | `PUBLIC` | 15 per player; never removed once placed |
| **Longest Road tile** (M-11) | unowned near board / held by a player | `PUBLIC` | Returns to the unowned pool when a route breaks and no single player qualifies (R-10.5) |
| **Largest Militia tile** (M-12) | unowned near board / held by a player | `PUBLIC` | Transfers only on strictly more Militia (R-10.6, R-10.8); can never return to the unowned pool once claimed |
| **Dice** (M-13) | idle / rolled | roll result `PUBLIC` | Also used to determine the first player (R-2.8) |
| **Player aid** (M-14) | with each player | `PUBLIC` (reference only) | No game state |
| **Card tray** (M-15) | table | `PUBLIC` | Organizational only |

### 4.3 Derived state (`DERIVED`)

| Item | Visibility | Notes |
|---|---|---|
| Player VP total | Partly public | Buildings and tiles are `PUBLIC`; hidden VP cards make the true total private until revealed (R-9.11) — track *apparent* and *actual* VP separately |
| Longest continuous route per player | `PUBLIC` | Computed from public road/building positions, forks excluded, opponent buildings breaking (R-10.3) |
| Militia played per player | `PUBLIC` | Faceup in player areas |
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
| `tiles` | holder of Longest Road / Largest Militia (or unowned) | `PUBLIC` |
| `players[n].hand.resources` | multiset of resource cards | `OWNER`, size `PUBLIC` |
| `players[n].hand.devCards` | list of unplayed development cards, each with `boughtOnTurn` | `OWNER`, size `PUBLIC` |
| `players[n].played.knights` | count of faceup Militia | `PUBLIC` |
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

Playing a Militia in `PRE_ROLL` and then rolling a 7 activates the robber **twice** in one turn — this is correct, not a bug (R-9.7 + R-6.3).

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
7. Each bonus tile is held by at most one player; Largest Militia requires ≥3 played Militia and Longest Road requires ≥5 continuous roads, in both cases strictly more than every other player.
8. `devCardPlayedThisTurn` ≤ 1 (excluding VP reveals).

### 5.6 Authority & randomness

Four randomness sources must be server-side and unforgeable: dice rolls, development deck shuffle, the random steal (R-6.4), and — in the Random Setup — hex and frame randomization. The steal is the one place where a card moves between private zones without either party choosing it; the victim must not learn what left their hand until they inspect it, and no other player may learn it at all.

### 5.7 Rules that are easy to get wrong

- **Supply exhaustion** (R-5.6): all-or-nothing, *except* when exactly one player is affected. Check before distributing.
- **Robber must move** (R-6.3): "stay put" is never legal; the desert is a legal destination.
- **Two robber entry points** (R-6.3, R-9.7) share one resolution routine, and can both fire in the same turn.
- **Forks don't count** toward route length (R-10.3), and your own buildings don't break your route.
- **Ties never transfer a bonus tile** (R-10.6).
- **Victory is only checked on the active player's turn** (R-11.1) — losing or gaining a tile during someone else's turn cannot end the game.
- **Trade is turn-gated** (R-7.3) and must be a genuine two-sided exchange (R-7.5). The trade *protocol* — offer lifecycle, binding, concurrency — does not follow from the rules at all and is a design decision (R-7.19, open market), with the concurrency consequences in §12.1.
- **Cities replace settlements** (R-8.7); the freed settlement returns to the pool and is reusable.

---

## 6. Engine architecture

### 6.1 Principle: library-first, not service-first

The engine is a **pure Rust library** — no I/O, no async, no networking in the core. Every consumer is a thin adapter around it.

This matters most for the AI-training use case. A single engine step should land in the low single-digit **microseconds**; an HTTP round-trip costs tens to hundreds of microseconds at minimum. Putting a network boundary on the training hot path would make serialization and syscalls dominate wall-clock time and render the language choice underneath irrelevant. "API first" here means *stable, well-specified interface* — the action catalogue in §5.4 — not *service first*.

### 6.2 Crate layout

| Crate | Responsibility | Depends on | Status |
|---|---|---|---|
| `carranta-core` | State, rules, legal-move generation, action application, recording seam. Pure, deterministic, no I/O | nothing | **built** |
| `carranta-bot` | Heuristic policy, self-play driver, market settlement | core | **built** |
| `carranta-record` | Log, replay driver, snapshot & seek, per-viewer redaction | core | **built** |
| `carranta-analytics` | Dice fairness, production decomposition, descriptives, rating | core, record | **built** |
| `carranta-evolve` | Population loop, parallel evaluation, versioned ladder, checkpoints (§9.5) | core, bot, analytics, record | **built** |
| `carranta-py` | PyO3 bindings: batched environments, observation encoding, action masks | core | |
| `carranta-server` | HTTP/WS service, matchmaking, persistence | core, record | |
| `carranta-wasm` | Browser bindings for the client | core, record | |
| `carranta-analytics` | Parquet/Arrow export, derived-event materialization | record | |

The dependency direction is strictly one-way: everything depends on `carranta-core`, and `carranta-core` depends on nothing. If the core ever needs a network or database type, the design has gone wrong.

### 6.3 State representation

- **Fixed-size and `Copy`.** From the §5.2 zones — 19 hexes, 54 intersections, 72 edges, ≤4 players, pools, hands, deck, turn flags — the whole state is on the order of **~300 bytes** with no heap allocation. Cloning a state for an MCTS node is a `memcpy` of a few cache lines, effectively free.
- **Bitboards.** 72 edges fit in a `u128` and 54 intersections in a `u64`, one per player. Legal road generation becomes `expand(own_roads) & !occupied & !blocked_by_opponents` — a handful of bit operations rather than a graph walk. This is where the order of magnitude over a scripting language comes from.
- **Enum state machine.** The §5.3 phases and §5.4 actions are an enum-and-`match` problem. Exhaustive matching means adding a phase without handling a transition is a compile error, which is the right failure mode for a rules engine whose bugs otherwise manifest as subtly illegal states.
- **Hot spot: longest route** (R-10.3). The only nontrivial algorithm in the game, run on every road placement, made a genuine path search by the forks-don't-count rule. Built and measured — see `engine-performance.md`. The decisive insight was that the board's adjacency is already implicit in precomputed bitmasks, so the common case never builds a graph at all. Incremental caching remains the largest outstanding win and belongs in the engine, not the module.

### 6.4 Determinism

Determinism is required for reproducible training, debuggable replays, and the snapshot-verification invariant in §7.6.

- **Split RNG streams** — separate seeded generators for dice, deck shuffle, the random steal (R-6.4), and setup randomization. Holding one fixed while varying another is essential when debugging and when running paired evaluations.
- **No incidental nondeterminism.** `std::HashMap` randomizes its seed per process; use `BTreeMap` or a fixed hasher anywhere iteration order can reach game state.
- **Version stamping.** Every game records `engine_version` and `rules_version`. With six design decisions and four derived rules outstanding, the rules *will* change, and old data must remain interpretable.

### 6.5 AI training interface

- **Batched environments.** Crossing the PyO3 boundary once per step costs more than the step itself. Step *N* games per call, EnvPool-style, with observations written directly into caller-provided numpy buffers.
- **Action masks in Rust.** RL needs a legal-action mask every step; generating it in Python would negate the engine's speed. This is a primary consumer of the bitboard representation.
- **Per-seat observations** are generated from the §4 visibility model — the same classification that drives replay redaction (§7.3). One implementation, two consumers.
- **Determinization for imperfect-information search.** Carranta is not a perfect-information game, so tree search needs opponents' hidden state resampled consistently with public history. The four items in §4.4 are exactly and completely what must be resampled — that list is the specification.
- **Trade mode is configurable.** D-5's open market gives an unbounded, combinatorial trade action space *and* lets non-active players act, which breaks the clean turn-based MDP most RL machinery assumes. Published Carranta RL work generally disables or heavily restricts trading. The engine therefore exposes trade policy as a dimension — `full` (open market, for human play), `restricted` (a small fixed offer menu), `disabled` — rather than hard-coding R-7.19. **Build this seam now; retrofitting it later means touching every layer.**

### 6.6 Performance targets

Full table, method notes, and the measured baseline live in
[`engine-performance.md`](engine-performance.md). Summary:

| Operation | Target | Measured |
|---|---|---|
| Longest road, realistic 15-road network | ≤ 100 ns | **91 ns** — met |
| Longest road, four-player sweep | ≤ 400 ns | **293 ns** — met |
| Longest road, dense/adversarial network | ≤ 500 ns | **1 455 ns** |
| Whole game, all seats current throughout | ≤ 10 µs | **6.8 µs** — met |
| Apply one action | ≤ 50 ns | **~35 ns** — met |
| Legal move generation | ≤ 200 ns | **~22 ns** — met |
| State clone | ≤ 20 ns | **~6 ns** (384 B) — met |
| Full random game | ≤ 50 µs | **~130 µs** — but see action-count note |
| Full game, competent play | ≤ 50 µs | **~59 µs** at ~479 actions |
| Self-play, one core | ≥ 20 000 games/s | **~7 700** |
| Bot win rate vs random | ≥ 99% | **99.81%** — met |

The full-game target sets everything else: ~300 actions in ≤ 50 µs is ~160 ns
per action including production, legality and scoring.

**Nothing in this table may be quoted as a fact about the engine until it has a
measured value.** The three that do are from a batched benchmark in the repo,
reproducible with `cargo run --release --example bench_longest_road`.

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
| H-8 | Retention | **Game logs indefinite, chat bounded.** Logs are pseudonymous and are the analytics and training corpus; chat carries the personal content and expires after 90 days |
| H-9 | Object store | **Self-hosted MinIO.** S3-compatible, no storage vendor — durability and backup are therefore ours |

**Why H-1 matters most.** Recording resolved randomness rather than just a seed decouples stored games from any single engine build. A seed-only log requires bit-exact determinism *forever* — and with six design decisions and four unverified derivations still in play, a rules correction would silently reinterpret every historical game rather than failing loudly. Explicit outcomes make replay a pure fold over data.

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

**How the built log differs from this sketch, and why** (`carranta-record`):

- **Randomness rides on the decision that resolved it**, as `Decision{action, resolved}`, rather than arriving as its own event. A roll *is* its dice; separating them creates an ordering question a replayer would have to answer, and answering it wrongly is silent. Category 4 survives as the `resolved` field.
- **Board generation and deck order live in `GameCreated`**, not in play events. Both are drawn once at state construction, so a log that stores the opening state already contains them — and then only two things resolve randomly during play at all, the dice and the robbery. `DevCardDrawn` becomes a *derived* event (H-7): the deck is known, so the card drawn is a fold, not a fact needing storage.
- **Snapshots are a side index, not an event category.** They are regenerable from the events, so keeping them out of the stream is what makes the stream canonical (H-7). They double as the checksum: `verify()` replays into every one.
- **Proposals, acceptances and withdrawals are ordinary decisions**, because in this engine they change state (the market is state). Only *declining* needs its own event, since it changes nothing and would otherwise leave no trace.

### 7.3 Replay and redaction

Replay is `fold(apply, events)` from a snapshot or from the start. Snapshots give seeking without replaying from event zero.

Redaction derives entirely from §4: `PUBLIC` data is served to everyone, `OWNER` data only to its owner, `HIDDEN` data to no one. The four items in §4.4 are exactly what must be masked.

Two things about redaction are easy to get wrong:

- **It is a function of `(event, viewer, time)`, not a static classification.** A card that is `OWNER` when drawn becomes `PUBLIC` when played; VP cards are hidden until the winning reveal (R-9.11); Monopoly forcibly exposes part of every hand (R-9.9). A redaction layer that classifies by event type alone will either leak or over-hide.
- **It must run server-side, before serialization.** Never ship a full log to a client and filter in the UI. Live spectating and mid-series replay sharing both depend on this path being correct, which makes it security-critical rather than merely cosmetic.

**The built answer: project the position, do not mask the event.** `carranta-record::fog` replays the log omniscient and emits, per event, the *position* a viewer is entitled to know. That resolves the first hazard by construction rather than by care — the thief's own hand shows the card they took, the table sees only that hand sizes moved, and Monopoly needs no special case at all. It also forces the second, since projecting requires the true state and therefore cannot happen client-side.

The redacted type has **no field** for another seat's card identities and none for the deck order, so a leak is a compile error rather than a missed branch. What cannot be expressed cannot escape: a stolen card's identity has no representation in the served form.

Tests assert *indistinguishability* rather than checking fields — swap two cards between two hands, or reverse the undrawn deck, and every other viewer's projection must be byte-identical. That is the property §7.6.2 actually wants, and unlike a field-by-field check it does not silently stop covering a field added later.

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

**Aggregation hazard:** because design decisions and options like R-3.12 change actual gameplay, games are not homogeneous. Every aggregate must be filterable by `rules_version` and game options, or analyses will silently mix incomparable games. Make those columns mandatory rather than nullable.

### 7.5 Sizing

~~Rough estimates to validate, not measurements.~~ **Measured** over 300 recorded self-play games per mode (`cargo run --release -p carranta-record --example bench_record`):

| | Trading off | Open market |
|---|---|---|
| Events per game | 510 | 708 |
| Snapshots per game | 8.5 | 11.6 |
| In-memory size | 27.9 KB | 38.6 KB |
| Cost of recording a game | within noise of not recording | within noise |
| Replay a game | 31 µs | 36 µs |
| Verify (replay + every snapshot) | 33 µs | 36 µs |
| Project one seat's whole view | 223 µs | 276 µs |

The original estimate holds. "A few hundred state-changing actions" was right, and the open market multiplies events by 1.4× rather than several-fold — the R-7.20 cap and the bot's per-request toll (§9.3) both bite. In-memory size is dominated by `Event`'s 48 bytes of padded enum, not by content; a packed wire encoding is the H-5 work and should land near the estimated low single-digit KB.

Two results worth carrying forward:

- **Recording is free.** It is an observer: no extra allocation per action beyond the push, and a test asserts a recorded game is the same game as an unrecorded one. So H-3's "off for self-play unless requested" is a storage decision, not a speed one.
- **Replay is ~55× cheaper than play.** Re-folding a corpus for a changed metric costs ~36 µs per game, so a million games regenerate in well under a minute on one core. That is what makes H-7's "derived events are a materialized view" practical rather than aspirational — nothing derived needs to be stored carefully, because recomputing it is cheap.

### 7.6 Risks

1. **Trade churn.** R-7.19 lets any player propose at any time during a turn, and H-4 records all of it. Bounded by R-7.20 (D-7): ~20 offers per player per turn plus a minimum interval, enforced in the engine. Without that cap this is a log-bloat and denial-of-service vector, not merely a tidiness concern.
2. **Redaction leaks are silent.** ~~Add an explicit test~~ **Done, and stronger than originally specified.** Rather than asserting that no `OWNER` or `HIDDEN` datum *appears* in another seat's view — which only ever covers the fields someone remembered to check — the tests perturb hidden state and assert the view does not move: swap two cards between two hands, or reverse the undrawn deck, and every other viewer's projection is byte-identical. Combined with a redacted type that has no field for the hidden data, a leak now takes a deliberate change rather than an oversight.
3. **Replay divergence.** Verify replayed state against the checksum in each `StateSnapshot`. A mismatch means either a corrupted log or an engine change that altered semantics — both need to fail loudly, immediately.
4. **Rules drift across the corpus.** See the aggregation hazard in §7.4.
5. **Identity and privacy.** H-6 creates durable cross-game player records. Retention is settled by H-8 — logs indefinite, chat 90 days — which is what makes a deletion request satisfiable without touching immutable game history. Pseudonymisation of the principal table still needs a concrete design.
6. **Storage durability.** H-9 self-hosts MinIO, so replication and off-box backup are ours. Game logs are simultaneously the source of truth and the training corpus; losing them is unrecoverable. Do not put real games in a single-node deployment without a backup plan.

---

## 8. Platform scope

The engine (§6) is one component of a product: accounts, lobbies, matchmaking, spectating, and chat sit above it. Nothing in this section may leak into `carranta-core`.

### 8.1 Decisions

| # | Area | Decision |
|---|---|---|
| P-1 | Guest identity | **Device-persistent and claimable.** A guest carries a durable ID and can later attach email or Google, keeping full history |
| P-2 | Disconnect / abandonment | **Bot takeover after a timeout.** The game continues; the substitution is recorded |
| P-3 | Pacing | **Real-time only** for v1. All players present, turn timers |
| P-4 | Authentication | **Self-hosted / self-owned**, not a managed identity vendor — settled as Auth.js in P-15 |
| P-5 | Discovery | **Browsable lobby list.** No matchmaking queue in v1 |
| P-6 | Spectators | **Allowed, with fog** — public observer view only |
| P-7 | Communication | **Text chat in v1**; voice designed-for-later, not built |
| P-8 | Chat data | **Recorded in a separate stream**, with its own retention class (90 days, H-8) |
| P-9 | Client | **Responsive web app.** One codebase, desktop and mobile browsers, invite links open straight into a lobby |
| P-10 | Hosting | **Railway.** Managed deploys with WebSocket and TLS support |
| P-11 | Minimum age | **16+**, self-declared at signup and for guests — chat is the trigger, not the account |
| P-12 | Localisation | **English only, i18n-ready.** Every user-facing string routed through a translation layer from day one |
| P-13 | Accessibility | **Standard palette with a colourblind mode** — alternate palette plus non-colour encodings, selectable in settings |
| P-14 | Rating in lobbies | **Display only.** Ratings shown, never used to gate joining |
| P-15 | Auth | **Auth.js**, owning our own principal table |
| P-16 | Moderation | **Wordlist filter plus report queue**, with the retained chat log as evidence |
| P-17 | Turn timers | **Configurable per lobby**, offering all three modes. Bot-only games are untimed |
| P-18 | Disconnects | **60–90s grace**, then bot takeover; the seat is **reclaimable** on reconnect |
| P-19 | Spectator delay | **None.** Spectators hold strictly less information than players, so relaying gains them nothing |

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
- **Turn timer mode is a lobby setting** (P-17), offering all three: per-decision reset (~60–90s, generous, never cuts off a multi-action turn), a chess-clock per-player budget for competitive play, or a short strict per-turn limit for brisk casual games. **Bot-only games run untimed** at engine speed — which is exactly what self-play and evaluation need.
- On expiry the turn auto-resolves (forced actions only) or the seat passes to a bot per P-2.
- **Ratings are shown but never gate joining** (P-14), so every listed lobby stays joinable — which is what keeps a thin player pool from looking dead.
- **Accepted consequence of P-3:** live state may assume all players are present. Adding correspondence play later would mean re-architecting live state handling, not just extending timeouts.

### 8.4 Spectators — and what P-6 costs

Choosing fogged spectating over no spectating **moves §7.3 redaction from a post-game convenience onto the live path, in v1**. Three consequences:

1. **The redaction leak test (§7.6, risk 2) becomes a launch blocker**, not a later hardening task. A bug there now exposes live hands to strangers rather than mis-rendering an old replay.
2. **Spectator view is the neutral observer view**: `PUBLIC` data only, no seat's `OWNER` data, ever. It is not "a player view minus that player" — it is strictly less than any player's view.
3. **Collusion risk is smaller than it first appears** (P-19). Because a spectator's view is strictly *less* than any player's, they cannot relay information a player could not already see. The residual is attention assistance — noticing a hand size or a probability the player missed — which is coaching, not leakage. No broadcast delay is applied; revisit only if rated play ever carries stakes.

Spectators are not seats: they hold no `player_id` in the game log and never appear in `fact_game_player`.

### 8.5 Chat

Text chat in v1 (P-7), per-lobby and per-game channels. Under R-7.19's open market, negotiation is the core social loop, and structured offers alone would leave it flat.

**Storage (P-8).** Chat lives in its own stream keyed by `game_id` and correlated to the game log by sequence number and timestamp, so replay can interleave conversation with actions without embedding personal data in the canonical corpus. This is what makes a deletion request satisfiable without rewriting immutable game logs — the reason to accept the small cost of correlating two streams.

**Moderation (P-16):** per-player mute and block, a report flow, and a maintained wordlist filter applied server-side. The wordlist catches the obvious cases at no latency or cost; human review of reports handles the rest, with the retained chat log (H-8, 90 days) as the evidence trail. That evidentiary role is a substantial part of why chat is recorded at all. A 16+ policy (P-11) is an assertion of duty of care, so reactive-only moderation was not an option.

**Voice (deferred).** Keep the transport abstraction voice-ready without building it. For ≤4 participants a WebRTC mesh is viable; beyond that, an SFU or a vendor (LiveKit, Daily, Agora). **Voice is not recorded** — the consent, storage, and jurisdiction burden is disproportionate, and none of the analytics goals need it.

### 8.6 Authentication (P-15)

**Auth.js** (P-15), running in the web client, with the principal table owned by us (§8.2). Chosen over a standalone identity server because guest-to-account claiming becomes our own code against our own schema rather than a fight with someone else's user model — and it is one less service to operate.

Three consequences to plan for:

- **Password flows are ours to build.** Auth.js deliberately ships no batteries-included email/password path, so hashing (Argon2), verification mail, and reset tokens are security-sensitive code we write. Leaning on Google OAuth as the primary route reduces how much of this is on the critical path.
- **Cross-language session validation.** The game server is Rust and Auth.js lives in the web app. Issue JWTs the Rust server verifies against a shared JWKS, rather than calling back into the JS app on every WebSocket connect.
- **It picks the client framework.** Auth.js runs inside a JS meta-framework, so P-9's web app is a Next.js or SvelteKit application by implication.

### 8.7 Client (P-9, P-12, P-13)

A **responsive web app** — one codebase for desktop and mobile browsers. This matters more than it looks: invite links (§8.3) and guest play (P-1) both depend on a shared URL putting someone at a table with nothing to install. Auth.js (P-15) implies a JS meta-framework, so the client is a Next.js or SvelteKit application rendering a `carranta-wasm` game view.

**Localisation (P-12).** English only at launch, but every user-facing string goes through a translation layer from day one and none are hard-coded in components. Adding a language then becomes a content task rather than a refactor.

**Accessibility (P-13).** Carranta leans on colour twice — four player colours, and red-marked high-probability numbers — and red/orange player pieces collide directly for the ~8% of men with red-green colour vision deficiency. Ship the standard palette with a **colourblind mode** in settings: an alternate palette that separates under deuteranopia and protanopia, plus a second encoding channel (a glyph per player, a non-colour marker on the 6 and 8). Two consequences to accept: two palettes to maintain and test, and a setting players must find before the game becomes fully playable for them — so surface it at first run rather than burying it.

---

## 9. Bots and the LLM player

### 9.1 Decisions

| # | Area | Decision |
|---|---|---|
| B-1 | Lineup | **Heuristic + LLM first**; a trained agent later |
| B-2 | LLM output budget | **Adaptive** — index-only by default, brief reasoning on decisions that carry the game |
| B-3 | LLM access | **Internal and flagged accounts only.** Not in public lobbies |
| B-4 | LLM purpose | **All four**: stand-in, live opponent, evaluation baseline, bootstrap training data |
| B-5 | Trained agent | **Neuroevolution first, on one machine** (§9.5). Gradient methods stay open as a later track |
| B-6 | LLM tier | **Two pinned models**: a small fast one for index-only decisions, a stronger one for the decisions in B-2's reasoning list |

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

**Cut the call count before optimising the call.** The engine should **auto-resolve forced decisions** — any state with exactly one legal action — without consulting any player. Much of a Carranta game is forced or near-forced, so this removes most LLM calls outright, and it benefits RL rollouts identically.

**Two models, routed (B-6).** The cheap fast model handles index-only decisions, which carry the volume; the stronger model is invoked only for the decisions listed below. Both are pinned, so B-4's benchmark role survives — two versions to track instead of one.

**Where the reasoning budget goes (B-2).** Index-only everywhere except: initial placement (R-3.7, R-3.8), robber placement and victim choice (R-6.3, R-6.4), trade evaluation, and development card timing. These are where Carranta games are actually decided; everything else is bookkeeping.

**Trade mode must be `restricted`.** R-7.19's open market makes the legal action list unbounded — all possible offers cannot be enumerated into a prompt. LLM play uses the same `restricted` seam that RL needs (§6.5), which is the second independent reason that seam has to exist before either is built.

**Pin everything for B-4's evaluation role.** Model ID, version, temperature, and a hash of the prompt template are recorded as part of the seat's agent identity (H-6). A silently updated model otherwise invalidates every prior benchmark without any signal that it happened.

**Capture rationales.** When B-2 produces reasoning tokens, store them in a side stream keyed to the event sequence — the same pattern as chat (P-8), and directly useful later for distillation into a trained agent.

### 9.5 Trained agent (B-5) — neuroevolution on one machine

The engine turned out fast enough to pull training forward. Rather than waiting on cloud infrastructure and a gradient stack, the first trained agent is evolved locally: **no GPU, no cluster, no gradients — one laptop, all cores, for days at a time.**

**This requires no new engine capability.** Action masks, microsecond steps and forced-move auto-play are already specified for other reasons. What it needs is a population loop, an evaluation harness, and a progress measure — and §10.5 already supplies the third.

#### Decisions

| # | Question | Decision |
|---|---|---|
| E-1 | Method | **Evolution strategy over the existing weights first, then NEAT.** The cheap step de-risks the expensive one and shares its whole harness |
| E-2 | Hardware | **One machine, every core, no accelerator.** Evaluation is embarrassingly parallel and the networks are small enough that a GPU would idle |
| E-3 | Network inputs | **Engineered features, not raw board.** NEAT complexifies badly with hundreds of inputs |
| E-4 | Evaluation | **Paired trials on common random numbers**, variants mirrored across seat pairs |
| E-5 | Evaluation budget | **Grows as the population converges.** A fixed budget is wrong at both ends |
| E-6 | Fitness | **Mean finishing position**, not win rate — the full order, for the same reason §10.5 uses it |
| E-7 | Opponents | **Fixed anchor plus a hall of fame.** The current heuristic, pinned, and past champions |
| E-8 | Progress measure | **The §10.5 rating, with the heuristic's μ frozen** as an absolute reference |
| E-9 | Trade mode | **Restricted.** The cheapest setting a strategy can transfer out of; measured at ~3.5× the cost of trading off |
| E-10 | Champion rating | **Held-out games.** A champion is never rated on the games that selected it |
| E-11 | Anchor rating | **μ pinned, σ free.** The reference defines the scale rather than moving on it |
| E-12 | Rating configuration | **Rate where people play.** An agent trains in `Restricted` and is rated in the human pool, or its rating is not comparable to a person's (§10.8) |
| E-13 | Checkpointing | **After every generation, atomically, in plain text.** An interruption costs one generation, never a run |
| E-14 | Behavioural markers | **Sampled games run through §10.** A rating says something improved; only this says what |

#### What the measurements say

From `cargo run --release -p carranta-analytics --example bench_evolution`, on a 4-core x86 machine:

| Threads | Games/s | Speedup | Efficiency |
|---|---|---|---|
| 1 | 558 | 1.00× | 100% |
| 2 | 1 040 | 1.86× | 93% |
| 4 | 2 093 | 3.75× | 94% |

**Scaling is essentially free** — nothing shared gets in the way, which is what a `Copy` state and a dependency-free core buy. At ~543 games/s/core, eight cores play **~376 M games/day**, so a corpus in the billions is a matter of days rather than weeks.

That figure is measured on x86 and should not be quoted for an M1 without checking. An M1 laptop has more per-core throughput, but four of its eight cores are efficiency cores and the chassis is fanless, so sustained all-core output will not be eight times a performance core. **Plan on ~300 M games/day and measure the real machine before relying on it.**

#### The number that actually governs the design

Throughput is not the constraint. Variance is. Perturbing one weight of the heuristic and playing paired trials:

| Change | Effect on mean finishing position | Paired trials to resolve at 95% |
|---|---|---|
| Production weight ×3 | +0.266 | **37** |
| +50% | +0.105 | 207 |
| +17% | +0.032 | 2 092 |
| +8% | +0.009 | **26 859** |

*(Position, 1 = winner, so a positive effect means the change made it play worse. The hand-set production weight is at or above its local optimum.)*

Cost scales as **1/effect²**: halve the difference and the games quadruple. Hence E-5. Early generations, where genomes differ wildly, resolve in tens of trials and are nearly free; late-stage fine-tuning at the ±8% level costs seven hundred times more. A fixed per-genome budget either wastes compute early or stalls out late.

**Pairing is exact, which is why E-4 is a decision and not a detail.** Identical agents on the same board play the identical game, so the paired difference is *exactly* zero — board luck and seat effects are removed by construction rather than averaged away. Without it the trial counts above would be far larger.

Two further findings worth carrying:

- **The landscape has flat plateaus.** Quartering the victory-point weight changed almost nothing — it is still dominant at a quarter strength, and the games mostly play out identically. A local search that assumes a smooth response will waste its budget on regions where nothing responds.
- **A generation is cheap at every plausible budget.** 150 genomes at 1 000 trials each is 300 000 games — about a minute on eight cores. Even 5 000 trials each leaves ~250 generations per day. Generations are not the scarce resource; *resolution within a generation* is.

#### What the first runs showed

`cargo run --release -p carranta-evolve --example train` runs the loop. Over 16 generations on 4 cores — 28 224 games in 47 s, ~600 games/s with the market open — champions settle **+2 to +4 μ above the pinned heuristic**, with σ ≈ 2.7. Real, modest, and reached within a handful of generations before flattening out, which is what the plateau finding predicted.

Three things the build corrected, each of which would have quietly inflated the result:

- **Winner's curse (E-10).** Selecting the best of 48 genomes and then rating it on *those same games* made champions look **+10.8 μ** above the anchor. The best of N noisy estimates is biased upward by roughly the noise itself. Rating the champion on fresh seeds after selection cut it to +4.4 — the honest number, and 2.5× smaller.
- **A drifting anchor (E-11).** A reference that plays and loses sinks, so "+4 μ above the heuristic" meant something different in generation 40 than in generation 1. The anchor's μ is now pinned, making it an origin rather than a competitor. σ is deliberately left free, since it reflects games genuinely played. `a_pin_keeps_two_eras_comparable` demonstrates the failure and the fix.
- **Anonymous opponents.** Hall-of-fame versions were recorded without their identity, so old champions never accumulated games, their σ stayed wide, and the ladder could not separate an old version from a new one however long the run went. They now keep their identity and go on being rated.

**Ladder ratings are comparable across generations; fitness is not.** A generation's fitness is measured against a field drawn from that generation, which is itself improving — so flat fitness can mean steady progress, and a rise can mean an easier field. The ladder is the only cross-generation measure, which is exactly what E-8 is for.

#### Running it

Built to be started and left alone:

```
cargo run --release -p carranta-evolve --example train -- --out runs/first
cargo run --release -p carranta-evolve --example train -- --out runs/first --resume
```

Portable by construction — the whole workspace is `std` only, with no dependencies and no architecture-specific code, so an Apple-silicon laptop needs nothing added. `--threads` defaults to every core; on a fanless machine it is worth measuring four against six against eight, because sustained throttling can make more workers slower.

**Resume is exact, not approximate** (E-13). A generation's randomness is derived from `(run_seed, generation)` rather than carried in an evolving generator, so a checkpoint holds only numbers that can be written down — and a run resumed at generation 40 produces exactly the games it would have produced had it never stopped. A test asserts precisely that: three generations, save, reload, and three more that match a run which was never interrupted, generation by generation.

The format is plain text on purpose: a run that dies overnight should leave something readable, diffable and salvageable without the program that wrote it. Writes go through a temporary file and a rename, so a crash mid-write leaves the previous checkpoint intact rather than a half-written one. A file named `stop` in the output directory ends the run cleanly after the generation in flight.

`history.csv` gets a row per generation — fitness, noise, whether selection could separate the field, the champion's gap above the anchor **and its σ**, ladder connectivity, and the behavioural markers below.

**Behavioural markers** (E-14) close a gap the rest of the design left open: a rating that climbs says something improved, but not what. A few of each generation's validation games are recorded and run through §10, giving trades, offers, maritime trades, buildings by type, development cards, militia plays and production per generation. Selection never sees them, so they describe play rather than shaping it.

They earn their place immediately. Two findings came out of building them:

- **A negative `offer_discount` does not silence the bot, it inverts it.** The discount scales the *gain* a proposal is credited with; negative, the bot comes to prefer proposals whose gain is negative — deals bad for itself — and those are far more plentiful, so it gets *louder*. A fitness score would have called this "worse" with no hint of why.
- **`offer_cost` cannot quiet the first ask of a turn**, because the toll is charged per offer already made that turn. Raising it from its default to a punitive value changes nothing measurable. If offers ever need suppressing, `offer_discount` is the lever and this is not.

#### The reservation

NEAT extracts **one scalar per game**. A policy-gradient method extracts a learning signal from each of ~500 decisions in that same game. That gap is the real risk, and no amount of throughput closes it — which is why E-1 sequences a cheap, certain step ahead of the ambitious one, and why E-3 hands the network features rather than making it discover them.

What would change the plan: if evolution over the 14 existing weights stalls well short of what hand-tuning suggests is available, the flat-plateau finding is the likely cause, and the answer is a method that reads per-decision signal rather than a bigger population.

#### Later tracks, unchanged

Gradient self-play over the batched environments of §6.5 remains open, as does behavioural cloning from recorded LLM and human games — H-1 captures everything needed for it. Neither blocks the other: both consume the same player interface (§9.2) and are scored on the same rating scale (§10.5).

### 9.6 Cost control (B-3)

Internal and flagged accounts only for now. Before any wider exposure: per-game and global spend caps, and graceful degradation to the heuristic bot when a cap is hit or the provider errors. Guests must never be able to initiate LLM spend.

### 9.7 Risks

1. **Prompt injection through chat.** If free-text chat is ever placed into an LLM player's prompt, players can issue instructions to the bot — "give me all your wood" is a trade negotiation to a human and an instruction to a model. **Do not include chat text in the LLM player's prompt.** If social play later demands it, isolate it as clearly-delimited untrusted data and never as instructions. This is the sharpest interaction between §8.5 and this section.
2. **Latency shapes the game feel.** An LLM call per decision at human pace is tolerable; hundreds per game is not. Forced-move auto-play is the primary mitigation, and turn timers need to accommodate bot think time.
3. **Model drift breaks benchmarks** — see the pinning requirement above.
4. **Cost per game is unknown** until measured. Measure it on internal games before B-3 is relaxed.
5. **Bootstrap data quality.** B-4 uses LLM games as training data; a systematically weak LLM teaches those weaknesses. Validate against the heuristic baseline before using its games to warm-start anything.

---

## 10. Analytics and player rating

Everything here is derived from the canonical event log (§7) and is regenerable. Nothing in this section is computed inside `carranta-core`.

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
Length in turns and wall time · winner and final VP breakdown · roll histogram with §10.1(a) percentile · 7-count · total production by resource · robber moves and target hexes · steal matrix (who robbed whom) · trade counts by type (player, 4:1, 3:1, 2:1) · offers made/accepted/rejected · development cards bought and played by type · VP progression curve per player · Longest Road and Largest Militia holders over time and number of transfers · discards forced by 7s.

**Per player, within a game**
Expected vs actual production with the §10.2 decomposition · income by source (production, trade, Invention, Monopoly, steals) · outflow by sink (builds, trades, discards, robbed, Monopoly losses) · resources spent per VP earned · average and peak hand size · cards lost to discards · opening placement quality (pip count, resource diversity, port access) · trade profile (proposal rate, acceptance rate as proposer and as accepter, net resource balance per counterparty) · robber exposure (times targeted, cards lost) · think time by decision type.

**Per player, across games** *(requires H-6 identity)*
Games played, win rate, average finishing VP · win rate segmented by seat position, player count, setup variant, and rules version · opening preferences · build-order tendency (city-first vs expansion vs development cards) · trade behaviour and generosity · **luck-adjusted performance** (see §10.4) · rating and rating trajectory (§10.5).

**Corpus and balance**
Seat/turn-order win rate — the first-player advantage question · board layout imbalance · Fixed vs Variable setup outcomes · effect of the red-number option (R-3.12) · effect of trade mode · human vs bot and bot-version comparisons · the §10.1(b) RNG audit.

### 10.4 Luck-adjusted performance

Rating (§10.5) measures results; results in Carranta carry a large chance component. The complementary metric: **VP earned relative to what the player's production entitled them to.**

Concretely — regress final VP on total production (or on the §10.2 z-scores) across the corpus, and report each player's **residual**. A player who consistently finishes above the curve converted resources better than average; one below did not. This is the single most useful "were you good or lucky" number, and it is only computable because §10.2 gives an exact expectation rather than an estimate.

### 10.5 Player rating

**"Halo ranking algorithm" is TrueSkill** — Microsoft Research, developed for Xbox Live and first deployed on Halo 2. It is a good instinct for this problem, for a specific reason: **Elo is fundamentally a two-player system**, and Carranta is a 3–4 player free-for-all. Elo extensions to multiplayer are pairwise-decomposition hacks. TrueSkill models N-player outcomes natively and maintains a Gaussian belief `(μ, σ)` per player rather than a point estimate.

#### Decisions

| # | Question | Decision |
|---|---|---|
| A-1 | Model and implementation | **OpenSkill (Weng–Lin), Plackett–Luce variant.** TrueSkill-family behaviour, maintained open implementations, no patent exposure |
| A-2 | Pool segmentation | **One pool per major configuration** — (trade mode × major rules version). An agent is rated in the pool *people play*, whatever it trained in — see §10.8 |
| A-3 | Guest rating | **Rated provisionally**, high σ, shown as provisional, carried across on account claim |
| A-4 | Seat position | **Randomise seating** and report per-seat win rates across the corpus |
| A-5 | Leaderboards | **Account holders only**, above a games-played threshold so σ has converged |

*TrueSkill 2 (2018) adds margin and experience effects but has no public reference implementation. The patent position on TrueSkill proper is worth a legal check if A-1 is ever revisited — that is a flag, not advice.*

#### Design points

1. **Use the full finishing order, not just the winner.** Final VP totals rank all 3–4 players, so every game yields a complete ranking rather than one bit. Plackett–Luce consumes this natively (A-1), and it roughly triples the information per game — which matters given how slowly high-variance games converge.
2. **Display a conservative rating**, `μ − 3σ`, so new players aren't shown an inflated number before their uncertainty collapses. This is also what makes A-3's provisional guest ratings honest rather than misleading.
3. **Bots share the rating pool.** This is the baselining answer: a pinned heuristic bot with tight σ after thousands of games becomes an **absolute yardstick**. "Trained agent v4 at μ=32 vs heuristic at μ=25" is directly meaningful, and human ratings become comparable to bot ratings on one scale.
4. **Keep the set of "major" configurations deliberately small** (A-2). Every additional pool fragments player ratings and slows convergence, so a config should only earn its own pool when it genuinely changes how the game is played — trade mode does, a cosmetic option does not.
5. **Randomised seating (A-4) makes seat effects average out** over a player's games without modelling anything. It does *not* protect a player with very few games, so seat effects remain a known limitation at low game counts — visible in the per-seat corpus statistics rather than corrected in the rating.
6. **Exclude substituted games** (P-2) from rated updates — neither the departed human nor the bot that finished for them played a whole game.
7. **Guest ratings transfer on claim** (A-3) through the identity alias in §8.2 — never by rewriting historical games.
8. **Expect slow convergence.** Carranta's variance means σ shrinks slowly; show it rather than hiding it, and resist ranking players publicly before σ is small.

#### Known exposure

A-3 rates guests, and guest identity is a device-persistent ID (P-1). That combination is **smurf-friendly**: a player wanting a fresh rating can clear device state. Accepted for now — the alternative costs real signal from every guest game — but if rated play ever carries stakes (leaderboards, rewards, competitive matchmaking), revisit A-3 rather than trying to patch it downstream.

### 10.6 Statistical pitfalls to design around

1. **Multiple comparisons** on per-game dice tests — §10.1.
2. **Truncation bias.** Games end when someone reaches 10 VP, so "average VP at turn 25" includes only games that lasted 25 turns, biasing toward slow games. Report n per turn explicitly, or use survival-analysis framing.
3. **Players within a game are not independent.** One player's gain is literally another's loss; treating player-games as i.i.d. samples will understate variance in any aggregate.
4. **Configuration heterogeneity.** `rules_version`, trade mode, setup variant, and the R-3.12 option all change gameplay — mandatory filter columns, per §7.4.
5. **Bot games can swamp human data.** Self-play corpora are orders of magnitude larger; never pool them with human games without explicit segmentation.
6. **Survivor bias in player stats.** Players who quit early are underrepresented in long-run aggregates.

### 10.7 What was built, and what it measured

`carranta-analytics` implements §10.1–§10.5 over recorded games. `cargo run --release -p carranta-analytics --example report` runs the whole pipeline; the numbers below come from 1 000 self-play games under an open market, with the agents rotated around the table.

**The engine's own dice pass their audit** (§10.1b), on 115 595 pooled rolls:

| | |
|---|---|
| chi-squared | 5.8, p = 0.83 (10 df) |
| KL from theory | 0.000037 bits |
| Worst outcome gap | 0.116 percentage points |
| Share of sevens | 0.1661 against 0.1667 expected |
| Lag-1 autocorrelation | +0.0012 |
| Wald–Wolfowitz runs test | p = 0.96 |

The independence checks earn their place: a test asserts that a *sorted* roll sequence — perfect marginals, obviously not independent — is caught by the autocorrelation and runs test while the distribution checks wave it through. Marginal frequencies alone would have passed it.

**The per-game p-value is calibrated**, which is what makes §10.1's warning about it true rather than merely cautious. A test draws 400 fair games and asserts that about 5% clear p<0.05 — because they must, and because that is precisely why the number is reported to players as a percentile instead.

**The production decomposition holds exactly.** `Actual = E_raw − RobberCost − SupplyDenial + DiceLuck` is asserted as an identity to 1e-9 across every seat and every resource of 30 games — the check that the four terms are a decomposition rather than four separately-computed numbers that happen to sit near each other. Separately, dice-luck z-scores across a corpus come out centred near 0 with spread near 1, which is the test that the expectation is *right* rather than merely self-consistent: a wrong expectation would show as a systematic offset, not as luck.

Two things the implementation deviates on, both recorded in code:

- **Ties in the rating update behave oddly.** With four equal players a shared second place pays both tied players *less* than the average of second and third, and changes what fourth place loses even though fourth finished fourth either way. Total μ is conserved, so it is a redistribution question rather than a leak, and it appears to follow from the published tie-averaging convention rather than from a transcription error. It matters little today — only the active player can win (R-11.1), so the top position is never shared — but **A-1 should be cross-checked against a reference implementation before rated play carries stakes.**
- **Ratings are our implementation of the Weng–Lin update, property-tested rather than cross-validated.** Order monotonicity, σ that only shrinks, symmetry under a full tie, μ conservation under equal uncertainty, and convergence onto a known true ordering all hold. That is the honest limit of the assurance.

**Cost:** a full analysis — dice, production, descriptives, rating — is ~200 µs per game against ~7 600 µs to play one. Recomputing every metric over a million-game corpus is a few minutes on one core, which is what makes H-7's "derived events are a materialized view" a practical stance rather than an aspiration.

### 10.8 Benchmarking agents against people

The rating system is what turns human play into a benchmark, and most of the machinery is already there: versioned agents (E-8), a single pool, and a pinned anchor whose μ cannot drift (E-11). A trained agent and a person land on one scale by construction.

Three things stand between that and a trustworthy number.

**The pools would have kept them apart.** Training runs `Restricted` (E-9) and human play will run `Full` — and A-2 puts each configuration in its own pool, so an agent's rating and a person's would never have been comparable at all. The fix is a separation the design did not previously make: **an agent trains in one configuration and is rated in another.** Nothing stops a weights- or feature-based agent from *playing* the open market; the restriction exists to keep the generated action space enumerable while training, which is a training concern only. Registered as E-12.

**The obvious bridge is the wrong games.** P-2's disconnect takeover will produce human-versus-bot games in quantity — and §10.5's design point 6 excludes substituted games from rated updates, because neither the departed human nor the bot that finished for them played a whole game. So the games that arrive for free are precisely the ones that must be thrown away. The bridge has to be **deliberate**: bots seated in lobbies from the start, under their version identity, marked rated.

**How many bridge games it takes** (`cargo run --release -p carranta-analytics --example bridge`). Simulating the rating model from known skills, with each group playing 2 000 games among themselves and a varying number across:

| Bridge games | Claimed cross-group gap (μ) | Every cross pair ordered right |
|---|---|---|
| 0 | −4.7 | 0/12 runs |
| 100 | 3.6 | 0/12 |
| 200 | 7.5 | 4/12 |
| **400** | 10.0 | **12/12** |
| 800 | 11.8 | 12/12 |
| 1 600 | 12.0 | 12/12 |

*(True mean gap 6.0. μ sits on the rating's own scale — the update divides by `c ≈ 2β` — so a faithful rating settles near twice the generative gap, ~12 here.)*

**Roughly 400 games get the ordering right; 800–1 600 settle the size of the gap.** The striking part is the first row of the table that is *not* varied: within-group games are held at 2 000 throughout and contribute nothing. An agent that has played ten million games against other agents knows exactly where it stands among agents and nothing whatever about where it stands against people. **Only the crossing games count.**

That is affordable. With one seat in four given to a rated bot, 400 bridge games is about a week at fifty human games a day, and a day or two at a few hundred. It is also a reason to seat bots deliberately rather than hoping for incidental contact.

**Three threats to validity, all measurable from the logs rather than assumed away:**

1. **People play differently against a known bot** — less negotiation, more exploitation. That biases exactly the bridge games. Comparable by segmenting on whether the human knew, which the lobby record can carry.
2. **Who opts into a bot game is not a random sample of players.** Compare the bridge population's own ratings against the wider human population's.
3. **Humans are not stationary** — they improve, particularly early. τ exists for this, but a benchmark taken over a long window mixes a player's past and present selves.

None of these is a blocker, and none can be settled before there are human games to look at. What matters now is that the identity, pinning and segmentation decisions are made correctly *before* the first human game is recorded, because they cannot be retrofitted onto immutable logs.

---

## 11. Content to author

Four design assets are referenced by the rules but not yet specified. They are **ours to design** — none is a transcription task, and none blocks the engine, because each has a trivially valid default we can ship and tune later.

| ID | Asset | Needed for | Default if undesigned |
|---|---|---|---|
| ART-1 | **Port distribution and placement**: how many 3:1 and 2:1 ports, which resource each 2:1 serves, and which intersection pairs carry them | Both setup modes | 4× 3:1 and 5× 2:1 (one per resource), spaced evenly around the coast |
| ART-2 | **Number-disc placement order** for Random Setup — the traversal that R-3.3 walks | Random Setup | Spiral inward from a corner, skipping the desert, with the D-6 red-number check |
| ART-3 | **Beginner Setup board layout**: terrain and number for each of the 19 positions | Beginner Setup | — (mode unavailable until designed) |
| ART-4 | **Beginner Setup starting pieces**: the 8 settlement and 8 road positions, and which settlement is each player's "second" | Beginner Setup | — (mode unavailable until designed) |

**These are balance decisions, not data entry.** Port distribution in particular shapes the whole trading economy, and a beginner layout is a curated teaching board — both deserve playtesting rather than a first guess treated as final. Ship Random Setup first (fully specified by R-3), and treat the Beginner mode as a later content drop.

---

## 12. Rules decisions

Every rule in §2 is Carranta's own specification. This section records the rules that required a deliberate decision rather than following obviously from the system — the ones most likely to be questioned later, and therefore the ones most needing a written rationale.

### 12.0 Resolved edge cases

Situations a naive reading of §2 leaves ambiguous. Each is settled here and reflected in the rule tables above.

| # | Question | Ruling |
|---|---|---|
| 1 | Robber moved to a hex with no buildings? | Legal. The robber must move to a different hex; if no opponent has a building there, no card is stolen. Production on that hex is still blocked. |
| 2 | Robbing a player who has no resource cards? | No card is drawn, and the active player does not get to pick a different victim. |
| 3 | Must the robber move at all? | Yes. It must be placed on a *different* hex; the desert is a legal destination. |
| 4 | Can the robber block building or port trading? | No. It only blocks production on its own hex. |
| 5 | Bonus tile ties? | The tile stays with its current owner. Transfer requires strictly more. |
| 6 | Longest Road broken — who gets it? | Current holder keeps it if they still qualify; another player takes it if they now qualify; **no one** holds it if zero or multiple players tie for longest. It may be regained via a bypass. |
| 7 | Do forks count toward route length? | No. A route is a single continuous path between two intersections; own buildings don't break it, opponents' do. |
| 8 | Can Largest Militia be lost? | Only to a player with strictly more *played* Militia. Militia never leave the table, so the tile never returns to the unowned pool once claimed. Unplayed Militia count for nothing. |
| 9 | Monopoly — must players reveal hands? | No. Players must answer truthfully about their holdings but need not show cards. |
| 10 | Trading after a 7 is rolled? | Not allowed until the discard and robber resolution are complete; the turn then continues normally. |
| 11 | Militia before the roll, then a 7? | Both resolve — the robber is activated twice. Playing a development card is independent of the dice result. |
| 12 | Can a player voluntarily discard or gift cards to dodge the robber? | No. No voluntary reduction, no gifts, no one-sided trades, no credit. |
| 13 | Three-way / secret trades? | Both forbidden. Every trade is public and strictly two-party, with the active player as one party. |
| 14 | Use an opponent's port? | No. Only the owner of a building on a port may use it. |
| 15 | How many development cards per turn? | Buy as many as you can pay for; play at most one (VP cards excepted). |
| 16 | Can a player win outside their own turn? | No. Victory is checked and claimed only on the winner's own turn — but it is then immediate and irrevocable. |
| 17 | Settlement placement without a road connection? | Only during setup. Afterwards every settlement needs an own-road connection. |
| 18 | City on an empty intersection? | Never — cities only upgrade an existing own settlement. |
| 19 | Rebuild on an intersection freed by a city upgrade? | Yes, the returned settlement is reusable; buildings may never be *moved*, though. |
| 20 | Coastal intersections without a port? | Legal building spots. Any point where three hexes meet is an intersection. |

### 12.1 Design decisions

Seven questions where more than one rule would have worked and we chose. These are the rules most likely to be revisited, so each records what was chosen and why.

| # | Question | Decision | Rule |
|---|---|---|---|
| D-1 | Road Building with fewer than 2 legal placements (blocked board or short road pool) | **Place as many as are legal** (1 or 0). The card is discarded and the turn's development-card allowance is consumed. | R-9.10a |
| D-2 | Discard resolution order on a 7 | **Simultaneous.** All affected players discard at once; play resumes when the last confirms. | R-6.2a |
| D-3 | Taking a resource whose supply stack is empty (4:1/port trade, or Invention) | **Must pay in full.** The trade is illegal unless the stack can supply the whole amount; Invention takes as many as remain (possibly 1 or 0). | R-7.17 |
| D-4 | Same resource type on both sides of a multi-type player trade | **Forbid any overlap.** No resource type may appear on both sides of a trade. | R-7.18 |
| D-5 | Trade offer lifecycle | **Open market** — see D-5 notes below. | R-7.19 |
| D-6 | Red numbers (6/8) adjacent on a randomly generated board | **Game option**, defaulting to the constraint enabled. | R-3.12 |
| D-7 | Bounding trade offers under the open market | **Per-turn cap (~20) plus a rate limit**, enforced in the engine. | R-7.20 |

**D-5 notes — open market.** Multiple offers stay live at once and any player may accept any live offer at any point during the active player's turn. This is the most table-like option and the most state to manage; three consequences follow:

- Every live offer must still have the **active player as one party** (R-7.3). Offers between two non-active players are never valid, even in an open market.
- **Acceptance races are real.** Two players may accept offers the proposer can only cover once. Resolve atomically on a first-come basis, re-validating both parties' holdings at the moment of execution, and reject the loser with a clear reason rather than silently dropping it.
- Offers must be **re-validated, not merely displayed**, when the board or a hand changes. An offer that was legal when made can become illegal (cards spent on a build, a Monopoly played); it should be invalidated rather than executed against stale state.

**Still open — design confirmations** (not rules questions; they need playtesting or a second look):

1. **Building costs** (§2.8) are carried as a working set and have never been balance-tested for Carranta specifically. They are the single biggest lever on game pace — confirm by playtesting, not by assumption.
2. **Port distribution** (ART-1, §11) is a default, not a decision. It shapes the whole trading economy.
3. **The four inferred trade rules** — R-7.10, R-7.13, R-7.14, R-7.15 — follow from the system rather than being stated outright. R-7.15 (no trading before the dice roll) is the least certain, since a development card *may* legally be played pre-roll; confirm that the asymmetry is intended.

### 12.2 Trade rules — completeness audit

Trade is the most intricate area of the rule set and the easiest to leave with gaps. Status of every trade question identified:

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
| Player-trade ratios / multi-type trades | **Derived** — follows from the system, worth confirming | R-7.13 |
| Obligation to accept; when an offer binds | **Derived** | R-7.14 |
| Trading before the dice roll | **Derived** — placement in the Action phase implies no, but a *development card* may be played pre-roll, so the phase boundary is not simply "nothing before the roll" | R-7.15 |
| Bonus tiles tradeable | **Derived** (no) — they move only by meeting their own conditions | R-7.10 |
| Trading from an exhausted supply stack | **Decision** (D-3) | R-7.17 |
| Same type on both sides of a multi-type trade | **Decision** (D-4) | R-7.18 |
| Offer lifecycle / binding / concurrency | **Decision** (D-5) | R-7.19 |

Twelve trade questions are settled outright, four are derived from the system and worth an explicit confirmation (R-7.10, R-7.13, R-7.14, R-7.15), and three were genuine forks in the road — those are decisions D-3, D-4 and D-5.

The open-market decision (D-5) makes trade the most concurrency-sensitive part of the system: it is the only place where a non-active player initiates a state change, and the only place where two valid requests can race. Treat `ACCEPT` as a transaction against current state, never against the state the offer was authored in.

---

## 13. Next steps

Items struck through are **built**; see `engine-performance.md` for what each was measured at.

**Unblock (no dependencies, needed by everything)**

1. Specify **ART-1 and ART-2** (§11) — port distribution and disc placement order. Both have workable defaults, so this is balance work rather than a blocker. ART-3/ART-4 gate the Beginner mode only.
2. Close the three design confirmations in §12.1 — building-cost balance, port distribution, and the four derived trade rules.

**Engine** — ~~3–7 built~~

3. ~~Board topology module (19/54/72) with precomputed adjacency.~~ Generated at compile time in `build.rs`, validated against Euler's formula and the §5.5 invariants.
4. ~~`carranta-core`: bitboard state, enum state machine, legal-move generation, benchmark harness.~~ 68 tests; targets in §6.6 replaced by measurements.
5. ~~Every `R-x.y` rule as an acceptance test.~~ Including the six decision-backed rules, plus random playouts asserting invariants after every action.
6. ~~Random Setup.~~ Beginner Setup still waits on ART-3/ART-4.
7. ~~Trade mode seam (`full` / `restricted` / `disabled`).~~

**History and data** — ~~8–9 built~~

8. ~~Event log and replay path (§7.2, §7.3), built alongside the engine.~~ `carranta-record`; recording costs nothing measurable, replay is ~55× cheaper than play.
9. ~~Redaction leak test and snapshot verification.~~ Redaction asserts *indistinguishability* — perturb hidden state, and every other viewer's projection must be byte-identical. Still a launch blocker to re-verify against the real serving path once a server exists.
10. Own principal table and the guest-claim alias design (§8.2) before any account exists — retrofitting identity onto immutable logs is not possible.
11. Human-identity retention and deletion policy (§7.6 risk 5) before the first human game is recorded.

**Platform** — none built

12. Auth.js with our own principal table (P-15, §8.6) — including the JWT/JWKS seam to the Rust server, which is the part most likely to be underestimated.
13. Lobby lifecycle and config (§8.3), configurable turn timers (P-17), and seat-actor substitution with reclaim (P-18).
14. Web client on Railway (P-9, P-10), with the i18n layer (P-12) in place before the first strings are written.
15. Text chat as a separate stream (§8.5) with mute, block, report and the wordlist filter from the start — moderation is much harder to add to a live product than to build into it.
16. Colourblind mode (P-13) alongside the default palette, not after it.

**Bots**

17. ~~Heuristic bot.~~ 99.76% against random over 5 000 boards × 4 seats.
18. Forced-move auto-play in the engine (§9.4) — it cuts LLM call volume and evolution rollout cost simultaneously, so it now pays twice.
19. LLM player behind the B-3 flag, with the two-model routing of B-6, pinned model version and spend caps; measure real cost per game before considering wider access.

**Training** (§9.5, new)

20. ~~`carranta-evolve`: population loop, work-stealing evaluation across cores, versioned ladder.~~ Deterministic under any worker count; checkpoints are plain text.
21. ~~Evolution strategy over the fifteen existing weights (E-1).~~ Champions land +2 to +4 μ above the anchor and then plateau.
22. Feature encoder (E-3) — the observation NEAT actually sees. Reuse the heuristic's features as the starting set; this is the piece most likely to decide whether the whole track works.
23. ~~Pin the heuristic as an immutable rating anchor (E-8, E-11).~~
24. ~~Resume from a checkpoint.~~ Exact, atomic, plain text; a `stop` file ends a run cleanly.
25. **Seat rated bots in human lobbies deliberately** (§10.8), and mark disconnect-takeover games unrated. ~400 bridge games make an agent's standing against people meaningful; games the agent plays against other agents contribute nothing to it.
26. Population and mutation tuning. The current settings plateau within a handful of generations, which may be the landscape or may be the settings — the two are not yet distinguished.

**Analytics** — ~~27–29 built~~

27. ~~Expected-production engine (§10.2).~~ Four-way decomposition, asserted as an identity to 1e-9.
28. ~~Dice as an empirical percentile, with the pooled audit separate.~~ The engine's own dice clear the audit on 115 595 rolls.
29. ~~Rating (A-1…A-5).~~ With the caveat in §10.7: not cross-checked against a reference implementation, and its tie handling is counterintuitive and pinned but unresolved.
30. Encode the §10.6 pitfalls as constraints in the analytics layer — mandatory config filters are enforced by `Corpus::accepts`, but explicit per-turn *n* and the no-i.i.d.-pooling rule are documented rather than enforced.

**Decision register:** seven rules decisions (§12.1), nine data decisions (§7.1), nineteen platform decisions (§8.1), six bot decisions (§9.1), fourteen evolution decisions (§9.5), five rating decisions (§10.5) — 60 in total.

**The critical path is now the platform, not the engine.** Everything from the engine down through analytics is built and measured; nothing in §12–13 blocks the training track, and the training track blocks nothing else.
