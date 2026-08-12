# CATAN — Rules & Materials Scoping Document

**Source:** *CATAN — The Game* rulebook, CN3081, v6.250401 (6th Edition), © 2025 CATAN GmbH / CATAN Studio. 12 pages.
**Status:** Draft 1 — initial extraction. Covers the complete base-game rulebook.
**Purpose:** Establish a single reference for (a) the full rule set, (b) the complete material inventory, and (c) the information/visibility state of every item, as the basis for a digital implementation or rules engine.

**Conventions used here**

- Rules are numbered `R-x.y` so they can be referenced from tickets, tests, and code.
- Every rule is traceable to a rulebook page (`p.N`).
- Facts read from **diagrams/artwork** rather than rules text are marked *(image-derived)* — these should be confirmed against physical components before being treated as authoritative.
- Open questions are collected in [§7](#7-open-questions--gaps).

---

## 1. Game parameters

| Parameter | Value | Source |
|---|---|---|
| Players | 3–4 (in a 3-player game the white pieces are not used) | p.5 |
| Win condition | First player to reach 10 VPs **on their own turn** | p.2, p.10 |
| Turn order | Clockwise, starting with the first player | p.6 |
| Turn structure | Production phase → Action phase | p.6 |
| Setup variants | Fixed Setup (recommended for first game) / Variable Setup | p.4, p.11 |
| Designer | Klaus Teuber (1952–2023); ongoing design Benjamin Teuber | p.12 |

---

## 2. Rules

### 2.1 Objective (R-1)

| ID | Rule | Page |
|---|---|---|
| R-1.1 | The first player to reach 10 victory points (VPs) on their turn wins. | p.2 |
| R-1.2 | VPs are earned by building. Resources needed for building are collected and traded for. | p.2 |
| R-1.3 | VP sources: settlement = 1 VP, city = 2 VPs, Longest Route tile = 2 VPs, Largest Army tile = 2 VPs, each Victory Point development card = 1 VP, road = 0 VP. | p.8, p.9, p.3 |

### 2.2 Setup — Fixed Setup (R-2)

| ID | Rule | Page |
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

| ID | Rule | Page |
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

> Note: in setup round 2 the second road must be adjacent to the second settlement ("their second road on an empty adjacent edge"); the connection-to-own-network rule (R-6.2) is not applied to setup roads beyond that adjacency.

### 2.4 Turn structure (R-4)

| ID | Rule | Page |
|---|---|---|
| R-4.1 | Play proceeds in turns, starting with the first player, clockwise around the table. | p.6 |
| R-4.2 | A turn consists of exactly two phases in order: 1. Production phase, 2. Action phase. | p.6 |
| R-4.3 | After finishing the Action phase, if the player has not won, they pass the dice to the player on their left, who begins their Production phase. | p.6 |

### 2.5 Production phase (R-5)

| ID | Rule | Page |
|---|---|---|
| R-5.1 | **(Optional) Play a development card** before rolling the dice. | p.6 |
| R-5.2 | **Roll dice.** Roll both dice and add them. The total determines which hexes produce this turn. | p.6 |
| R-5.3 | **Production.** Every hex whose number disc matches the roll produces. Each player with a **settlement** on a producing hex receives 1 resource card of that hex's type from the supply. | p.6 |
| R-5.4 | A player with 2 or 3 settlements on the same producing hex receives 1 card per settlement. | p.6 |
| R-5.5 | A player receives **2** resource cards for each of their **cities** on a producing hex. | p.6 |
| R-5.6 | **Supply shortage.** If there are not enough cards of a produced resource in the supply to satisfy everyone's production, **no one** receives any of that resource. Exception: if only **one** player is affected, that player receives as many of those cards as remain in the supply. | p.6 |
| R-5.7 | Terrain → resource mapping: forest → wood, hills → brick, pasture → wool, mountains → ore, fields → wheat, desert → nothing. | p.6 |
| R-5.8 | A hex occupied by the robber does **not** produce resources when its number is rolled. | p.6 |

### 2.6 Rolling a 7 (R-6)

| ID | Rule | Page |
|---|---|---|
| R-6.1 | On a roll of 7, **no hex produces** any resources. | p.6 |
| R-6.2 | **Discard.** Each player (all players, not just the active one) holding **more than 7** resource cards must choose half of them, rounded down, and return them to the supply. *(Example: 9 cards in hand → discard 4.)* | p.6 |
| R-6.3 | **Activate the robber.** The active player **must** move the robber to a **new** hex (i.e., not the hex it currently occupies). | p.6 |
| R-6.4 | The active player then steals 1 **random** resource card from a player who has a building on the robber's new hex. If multiple players have buildings there, the active player chooses which one to rob. The card is taken without looking. | p.6 |
| R-6.5 | Development cards are never stolen by the robber. | p.9 |

> Not stated in the rulebook: what happens if no player has a building on the chosen hex, or if the chosen victim has an empty hand → see [§7](#7-open-questions--gaps).

### 2.7 Action phase — Trade (R-7)

| ID | Rule | Page |
|---|---|---|
| R-7.1 | Actions may be taken as often as desired and in any order, as long as the player has the resources. | p.7 |
| R-7.2 | The active player may trade freely with other players and with the supply. | p.7 |
| R-7.3 | During a player's turn, other players may only trade **with the active player** — not with each other and not with the supply. | p.7 |
| R-7.4 | **Player trade.** The active player announces which resource(s) they want and which they offer. Other players may accept, counteroffer, or make their own proposals. | p.7 |
| R-7.5 | **No gifting.** Cards may not be given away in any form. This includes trading matching resource types (e.g., 3 ore for 1 ore is not allowed). | p.7 |
| R-7.6 | **General supply trade (4:1).** Put 4 identical resource cards into the supply and take 1 card of a **different** resource. | p.7 |
| R-7.7 | **3:1 port trade.** With a building on a 3:1 port, put 3 identical resource cards into the supply and take 1 card of a **different** resource. | p.7 |
| R-7.8 | **2:1 port trade.** With a building on a 2:1 port, put 2 cards of the resource shown on that port into the supply and take 1 card of a **different** resource. | p.7 |
| R-7.9 | Port access requires a building (settlement or city) on that port's intersection. | p.7 |
| R-7.10 | Development cards may not be traded or given away. | p.9 |

### 2.8 Action phase — Build (R-8)

Building costs (player aid, *image-derived* iconography):

| Structure | Cost | VP |
|---|---|---|
| Road | 1 brick + 1 wood | 0 |
| Settlement | 1 brick + 1 wood + 1 wool + 1 wheat | 1 |
| City | 2 wheat + 3 ore | 2 |
| Development card | 1 wool + 1 wheat + 1 ore | ? (0 or 1) |

| ID | Rule | Page |
|---|---|---|
| R-8.1 | To build, return the required resource cards from hand to the supply. | p.8 |
| R-8.2 | **Roads** are placed on empty hex edges. A new road must connect to one of the player's existing roads or buildings. | p.8 |
| R-8.3 | A road may not be built starting on the far side of an **opponent's building** (an opponent building blocks route continuation through that intersection). | p.8 |
| R-8.4 | **Settlements** are placed on empty intersections, must satisfy the Distance Rule, and must connect to at least one of the player's existing roads. | p.8 |
| R-8.5 | **Distance Rule.** When placing a settlement, stay at least two edges away from all other buildings (own and opponents'). | p.8 |
| R-8.6 | A player has 5 settlement pieces; to build further settlements, one must first be upgraded to a city (piece limit is a hard cap). | p.8 |
| R-8.7 | **Cities always replace settlements.** Remove one of your settlements from the board, return it to your player area, and place the city on that intersection. | p.9 |
| R-8.8 | A player has 4 cities and may not build more. | p.9 |
| R-8.9 | **Development cards** are bought by drawing the top card of the facedown deck. | p.9 |
| R-8.10 | If the development card deck runs out, no more development cards may be built. Development cards never return to the supply. | p.9 |

### 2.9 Development cards (R-9)

| ID | Rule | Page |
|---|---|---|
| R-9.1 | Development cards stay **hidden** until played. | p.9 |
| R-9.2 | Development cards do **not** count toward hand size when a 7 is rolled, and cannot be stolen by the robber. | p.9 |
| R-9.3 | A player may play **at most 1** development card per turn, placing it **face up** in their player area. | p.9 |
| R-9.4 | A development card may not be played on the turn it was bought. | p.9 |
| R-9.5 | A development card may be played either **before rolling the dice** or at any time during the Action phase. | p.6, p.9 |
| R-9.6 | Development cards may not be traded or given away, and never go back into the supply. | p.9 |

**Card effects**

| ID | Card | Effect | Page |
|---|---|---|---|
| R-9.7 | **Knight** (14×) | Activate the Robber (R-6.3, R-6.4): move the robber to a new hex and steal 1 random resource card from a player with a building on that hex. | p.9 |
| R-9.8 | **Invention** (2×) | Take any 2 resource cards from the supply into hand — 2 of the same or 2 different resources. | p.9 |
| R-9.9 | **Monopoly** (2×) | Announce **one** resource type; every other player must give you all their resource cards of that type. Only one type may be named, regardless of how many cards are received. | p.9 |
| R-9.10 | **Road Building** (2×) | Build 2 roads at no cost (no resources spent). Normal road placement rules apply. | p.9 |
| R-9.11 | **Victory Point** (5×) | Worth 1 VP. Must be kept **hidden** in the player area unless revealing them reaches the VP total needed to win; then reveal all VP cards, including those built this turn. | p.9, p.2 |
| R-9.12 | **VP card exception.** Any number of VP cards may be played, even on the turn they were bought, in order to win — this bypasses R-9.3 and R-9.4. | p.2, p.9 |

### 2.10 Bonus tiles (R-10)

| ID | Rule | Page |
|---|---|---|
| R-10.1 | **Longest Route** (2 VPs). The first player with **5 continuous roads** in play receives the tile. | p.8 |
| R-10.2 | If another player has **more** continuous roads in play, they immediately receive the tile. | p.8 |
| R-10.3 | A route can be **broken** by an opponent building a settlement on an intersection within it, splitting it into two segments. | p.8 |
| R-10.4 | If a player's route is broken such that they no longer meet the requirement, the tile returns to the supply and stays there until a **single** player has the longest continuous route of at least 5 roads; that player immediately receives the tile and its 2 VPs. | p.8 |
| R-10.5 | **Largest Army** (2 VPs). The first player with **3 Knight cards in play** receives the tile. If another player has more Knight cards in play, they immediately receive it. | p.9 |

> Implied tie rule: the tile transfers only when a player has strictly *more* than the current holder; a tie leaves the tile where it is. For Longest Route the rulebook is explicit that after a break the tile is held in the supply until a single player is strictly longest.

### 2.11 Winning (R-11)

| ID | Rule | Page |
|---|---|---|
| R-11.1 | If a player has 10 or more VPs at any point **during their own turn**, the game ends immediately and they win. | p.10 |
| R-11.2 | To claim victory, the player turns over any number of Victory Point cards, including ones built that turn, to demonstrate reaching 10 VPs. | p.10 |
| R-11.3 | VP tally components: settlements (1 each), cities (2 each), Longest Route tile (2), Largest Army tile (2), VP cards (1 each), roads (0), Knights played (0 in themselves). | p.10 |

---

## 3. Materials — inventory

### 3.1 Board & terrain

| # | Item | Qty | Detail |
|---|---|---|---|
| M-01 | Sea frame pieces | 6 | Puzzle-piece ends with matching numbers; carry the ports |
| M-02 | Ports (printed on frame) | 9 *(image-derived)* | 4× 3:1 generic; 5× 2:1 (one each: brick, wood, wool, wheat, ore) |
| M-03 | Terrain hexes | 19 | 4× forest, 4× pasture, 4× fields, 3× hills, 3× mountains, 1× desert |
| M-04 | Number discs | 18 | 1×2, 2×3, 2×4, 2×5, 2×6, 2×8, 2×9, 2×10, 2×11, 1×12. 6 and 8 printed in red. Backs are lettered A–R for the variable setup |
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

**Key asymmetry to model:** a player's resource hand is `OWNER` for identity but `COUNTABLE` for size — hand size must be publicly known because the discard-on-7 rule (R-6.2) depends on it. Development cards in hand are also `COUNTABLE`/`OWNER`, but are explicitly excluded from the 7-discard count (R-9.2).

### 4.2 Per-item state table

| Item | Location states | Visibility states | Notes |
|---|---|---|---|
| **Sea frame piece** (M-01) | assembled in frame | `PUBLIC` | Static after setup. Arrangement is fixed (R-2.1) or randomized (R-3.1) |
| **Port** (M-02) | on frame | `PUBLIC` | Access = has building on the port intersection; access status is public |
| **Terrain hex** (M-03) | in frame slot | `PUBLIC` (faceup once placed) | Momentarily `HIDDEN` while being drawn randomly in variable setup (R-3.2) |
| **Number disc** (M-04) | on a hex / off-board (desert has none) | `HIDDEN` (facedown, letter side up during variable setup) → `PUBLIC` (number faceup) | Variable setup explicitly uses a facedown ordered phase (R-3.3) |
| **Robber** (M-05) | on exactly one hex (starts on desert) | `PUBLIC` | Position is always public; blocks production on its hex (R-5.8) |
| **Resource card** (M-06) | supply stack / player hand / in-transit during trade | supply: `PUBLIC` (faceup stacks, count visible) · hand: `OWNER` + `COUNTABLE` · trade: `TRANSIENT` (revealed to trade partner on exchange) · stolen: `HIDDEN` to victim's knowledge of destination? no — taken at random, `TRANSIENT` to the thief only | Supply stack counts are public and matter (R-5.6). Steal is random and unseen by others (R-6.4). Monopoly reveals part of hands (R-9.9) |
| **Development card** (M-07) | facedown deck / player hand (unplayed) / player area (played, faceup) / played VP card (hidden until win) | deck: `HIDDEN` (order unknown to all; remaining count `PUBLIC`) · hand: `OWNER` + `COUNTABLE` · played: `PUBLIC` · VP card: `OWNER` until win, then `PUBLIC` | Never returns to supply (R-9.6). Knights played remain faceup and are publicly counted for Largest Army (R-10.5) |
| **Settlement** (M-08) | player supply (unbuilt) / on intersection / returned on city upgrade | `PUBLIC` in all states | 5 per player, hard cap (R-8.6) |
| **City** (M-09) | player supply / on intersection | `PUBLIC` | 4 per player, hard cap (R-8.8); always replaces a settlement (R-8.7) |
| **Road** (M-10) | player supply / on edge | `PUBLIC` | 15 per player; never removed once placed |
| **Longest Route tile** (M-11) | unowned near board / held by a player | `PUBLIC` | Can return to the unowned pool when a route breaks (R-10.4) |
| **Largest Army tile** (M-12) | unowned near board / held by a player | `PUBLIC` | Transfers only on strictly more knights (R-10.5) |
| **Dice** (M-13) | idle / rolled | roll result `PUBLIC` | Also used to determine first player (R-2.8) |
| **Player aid** (M-14) | with each player | `PUBLIC` (reference only) | No game state |
| **Card tray** (M-15) | table | `PUBLIC` | Organizational only |

### 4.3 Derived state (`DERIVED`)

| Item | Visibility | Notes |
|---|---|---|
| Player VP total | Partly public | Buildings and tiles are `PUBLIC`; hidden VP cards make the true total private until revealed (R-9.11) — a player's *apparent* VP total and *actual* VP total must be tracked separately |
| Longest continuous route per player | `PUBLIC` | Computed from public road/building positions, including opponent-building breaks (R-10.3) |
| Knights played per player | `PUBLIC` | Faceup in player areas |
| Resource hand size per player | `PUBLIC` | Required for the 7-discard rule (R-6.2) |
| Development-card hand size per player | `PUBLIC` | Excluded from discard count (R-9.2) |
| Supply stack counts per resource | `PUBLIC` | Drives the shortage rule (R-5.6) |
| Port access per player | `PUBLIC` | Derived from buildings on port intersections |
| Production map (number → hexes → adjacent buildings) | `PUBLIC` | Recomputed as buildings are placed |

### 4.4 Hidden-information summary

Exactly four things are hidden in this game:

1. **Resource card identities in hands** — counts public, identities private.
2. **Unplayed development card identities in hands** — counts public, identities private.
3. **Development deck order** — unknown to all.
4. **Number disc values during the variable-setup placement step** — a temporary hidden state (R-3.3).

Everything else — board, buildings, roads, robber, played cards, tiles, supply stacks, dice — is fully public.

---

## 5. Implementation notes

- **Piece limits are rules, not conveniences.** Running out of settlements gates further building (R-8.6); running out of development cards ends card purchases (R-8.10).
- **Supply exhaustion has a special rule** (R-5.6) that is easy to miss: all-or-nothing except when exactly one player is affected.
- **Two "activate the robber" entry points** exist (rolling a 7, R-6.3; playing a Knight, R-9.7) and share one resolution routine.
- **The robber must move to a different hex** every time it is activated — "stay put" is not legal.
- **Victory is checked only on the active player's turn** (R-11.1); passing 10 VPs during another player's turn (e.g., losing/gaining a bonus tile) does not end the game until that player's own turn.
- **Trade is turn-gated** (R-7.3) — a valid trade always has the active player as one of the two parties.
- **The no-gifting rule** (R-7.5) needs an explicit validator: both sides of a trade must be non-empty and must not include the same resource type on both sides.
- **A single "route" model** serves both road placement (R-8.2, R-8.3) and Longest Route computation (R-10.1–R-10.4), including the opponent-building break.

---

## 6. Content assets not in the rules text

These exist only as artwork and must be transcribed from the physical components or high-resolution scans before implementation:

- The exact **Fixed Setup** layout: which terrain hex and which number disc occupies each of the 19 board positions (p.4–5 diagram).
- The exact **Fixed Setup** starting positions of the 8 settlements and 8 roads (p.5 diagram).
- The **port layout** on each of the 6 sea frame pieces: which port type sits on which pair of intersections, and each piece's puzzle-end numbers (p.3 artwork).
- The **A–R letter mapping** on the number disc backs, i.e. which letter carries which number (p.3, p.11).

## 7. Open questions & gaps

Situations the rulebook does not explicitly resolve; each needs a documented decision before implementation:

1. **Robber on a hex with no buildings** — R-6.3 requires moving the robber to a new hex, but no rule covers the case where the destination has no adjacent buildings. Assumed: legal move, no theft occurs.
2. **Robbing a player with an empty hand** — no card is drawn. Assumed: no effect, and the active player may not then rob someone else.
3. **Monopoly against empty hands** — assumed: the announcement still happens and simply yields 0 cards.
4. **Road Building with fewer than 2 legal placements** — assumed: build as many as legally possible (0 or 1) and the card is spent.
5. **Bonus tile ties** — the tile stays with the current holder unless a player is strictly ahead; explicit for Longest Route (R-10.4), implied for Largest Army (R-10.5).
6. **Losing Largest Army** — no rule causes played Knights to leave the table, so the Largest Army tile can never return to the supply once claimed. Confirm.
7. **Discard resolution order on a 7** — simultaneous vs. clockwise. Assumed: simultaneous, no information dependency between discarders.
8. **Trades in the Production phase** — R-7.1/R-7.2 place trading in the Action phase; assumed no trading before the dice roll.
9. **Development card played before the roll, then a 7 is rolled** — a Knight played pre-roll resolves the robber first; a subsequent 7 triggers a second robber activation. Confirm this is intended (it follows from the rules as written).
10. **Building costs are read from player-aid iconography**, not from rules prose — verify against the physical aid.
11. **Port count and distribution** are read from component artwork — verify 4× 3:1 and 5× 2:1.
12. **Rulebook page count** — this extraction covers a 12-page document; confirm no supplementary pages (e.g., a separate almanac) belong in scope.

---

## 8. Next steps

1. Transcribe the four missing artwork assets in §6.
2. Resolve the open questions in §7 into explicit design decisions.
3. Derive a formal state schema from §4 (board topology: 19 hexes / 54 intersections / 72 edges, plus per-player and shared zones).
4. Turn each `R-x.y` rule into an acceptance test.
