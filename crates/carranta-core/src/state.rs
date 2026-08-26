//! Game state.
//!
//! One fixed-size `Copy` struct, no heap, no indirection. Cloning a state for
//! a search node is a `memcpy` of a few cache lines, which is what makes tree
//! search over this game cheap (§6.3).
//!
//! Board occupancy lives in bitboards. One `u128` per player for roads over
//! the 72 edges, one `u64` each for settlements and cities over the 54
//! intersections, so "which of my roads touch here" and "do I hold this
//! port" are single mask operations rather than scans.

use crate::rng::{Rng, Stream};
use crate::topology::{
    EdgeSet, HEX_COUNT, VERTEX_COUNT, VertexSet, edges_at, hex_vertices, neighbors, vertex_bit,
};

/// Seats supported. The rules allow 3–4 players (§1).
pub const MAX_PLAYERS: usize = 4;
/// Victory points needed to win (R-1.1).
pub const WINNING_VP: u32 = 10;

/// The five resources.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Resource {
    Brick = 0,
    Wood = 1,
    Wool = 2,
    Wheat = 3,
    Ore = 4,
}

pub const RESOURCES: [Resource; 5] = [
    Resource::Brick,
    Resource::Wood,
    Resource::Wool,
    Resource::Wheat,
    Resource::Ore,
];

/// The six terrains. Desert produces nothing (R-5.7).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Terrain {
    Hills = 0,
    Forest = 1,
    Pasture = 2,
    Fields = 3,
    Mountains = 4,
    Desert = 5,
}

impl Terrain {
    /// What this terrain produces (R-5.7).
    #[inline]
    pub fn yields(self) -> Option<Resource> {
        match self {
            Terrain::Hills => Some(Resource::Brick),
            Terrain::Forest => Some(Resource::Wood),
            Terrain::Pasture => Some(Resource::Wool),
            Terrain::Fields => Some(Resource::Wheat),
            Terrain::Mountains => Some(Resource::Ore),
            Terrain::Desert => None,
        }
    }
}

/// The five development cards (R-9.7 … R-9.11).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum DevCard {
    Militia = 0,
    VictoryPoint = 1,
    Monopoly = 2,
    RoadBuilding = 3,
    Invention = 4,
}

/// Composition of the development deck: 25 cards (§3.2).
pub const DEV_DECK: [(DevCard, u8); 5] = [
    (DevCard::Militia, 14),
    (DevCard::VictoryPoint, 5),
    (DevCard::Monopoly, 2),
    (DevCard::RoadBuilding, 2),
    (DevCard::Invention, 2),
];
pub const DEV_DECK_SIZE: usize = 25;

/// Resource cards of each type in the supply at setup (§3.2).
pub const SUPPLY_PER_RESOURCE: u8 = 19;

/// Piece pools per player (R-8.6, R-8.8).
pub const ROAD_POOL: u8 = 15;
pub const SETTLEMENT_POOL: u8 = 5;
pub const CITY_POOL: u8 = 4;

/// How much player-to-player trading a game allows (§6.5).
///
/// A configured dimension, not a rule: the open market is right for human
/// play, but its action space is unbounded, which is why reinforcement
/// learning and the LLM player need it narrowed or switched off.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum TradeMode {
    /// No player-to-player trading. Supply trade is unaffected.
    ///
    /// The default, because the generated action space is what search and
    /// training consume and neither wants an unbounded one.
    #[default]
    Disabled,
    /// Offers are limited to one card for one card, so the generated set stays
    /// small and enumerable.
    Restricted,
    /// The open market of R-7.19: any shape of offer, several live at once.
    Full,
}

/// A live trade offer.
///
/// `give` and `want` are from the proposer's side. Every offer has the active
/// player as one party (R-7.3): either they proposed it, in which case any
/// opponent may take it, or an opponent proposed it to them.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Offer {
    pub from: u8,
    /// The one seat this offer is addressed to, or `None` for the whole table.
    ///
    /// R-7.19's market is open by default, any entitled seat may take any live
    /// offer, and that stays the default. Addressing one is a *protocol*
    /// choice rather than a rule (nothing in R-7 mentions it), and it exists
    /// because "I'll trade you, and only you" is a normal thing to say at a
    /// table. R-7.3 still binds: an addressed offer must have the active player
    /// as one of its two parties.
    pub to: Option<u8>,
    pub give: [u8; 5],
    pub want: [u8; 5],
}

/// Offers that may be live at once.
///
/// An engine capacity, distinct from the per-turn cap of R-7.20: that limits
/// how many offers a seat may *make* in a turn, this limits how many may be
/// outstanding simultaneously.
pub const MAX_OFFERS: usize = 8;

/// Offers one seat may make per turn (R-7.20, D-7).
pub const OFFERS_PER_TURN: u8 = 20;

/// Largest side of a *generated* proposal, in cards, under
/// [`OfferShapes::SingleType`].
///
/// A bound on enumeration, not on legality: `apply` accepts any well-formed
/// offer, so a human client may compose whatever it likes and a bot may accept
/// it. Three covers essentially every offer real play produces.
pub const MAX_GENERATED_OFFER: u8 = 3;

/// How generated trade proposals are shaped.
///
/// The same distinction as [`MAX_GENERATED_OFFER`]: this bounds what the
/// engine *enumerates*, never what it *accepts*. `apply` takes any well-formed
/// offer under either setting, so changing the shape changes what bots see as
/// candidates and what a page could list, and nothing else.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum OfferShapes {
    /// A single resource type on each side, up to [`MAX_GENERATED_OFFER`]
    /// cards. The original behaviour, and the default: mixed shapes multiply
    /// the candidate list by an order of magnitude, and a served table sends
    /// its candidates to a browser.
    #[default]
    SingleType,
    /// Every affordable shape: mixed resource types, up to `give` cards
    /// offered and `want` cards asked. `give: None` bounds the offered side by
    /// the hand alone, so a seat may put its whole surplus on the market at
    /// once.
    ///
    /// The candidate count is the product of the two sides' multiset counts,
    /// less the overlapping pairs (R-7.18). At 2 and 2 that is at most a few
    /// hundred and usually far fewer, which is what training runs use; at
    /// `None` it scales with the hand, which is a setting to choose knowingly.
    Mixed { give: Option<u8>, want: u8 },
}

/// Port kinds. Index 0 is the generic 3:1; 1..=5 are the 2:1 ports, one per
/// resource, indexed by `Resource as usize + 1`.
pub const PORT_KINDS: usize = 6;

/// Where a turn currently is (§5.3).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    /// Placing a starting settlement. `round` is 0 or 1 (R-3.7, R-3.8).
    SetupSettlement { round: u8 },
    /// Placing the road that must adjoin the settlement just placed.
    SetupRoad { round: u8, from: u8 },
    /// Before the dice: a development card may be played (R-5.1).
    PreRoll,
    /// A 7 was rolled and players over the limit owe discards (R-6.2).
    Discard,
    /// The robber must move (R-6.3). `from_militia` distinguishes a card play
    /// from a rolled 7, which decides where the turn resumes.
    MoveRobber { from_militia: bool },
    /// Building, trading, and one development card (R-7, R-8).
    Action,
    /// Someone reached 10 VP on their own turn (R-11.1).
    GameOver { winner: u8 },
}

/// Complete game state.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct State {
    // ---- Board, fixed after setup ----
    pub terrain: [Terrain; HEX_COUNT],
    /// Production number per hex; 0 on the desert, which never gets one.
    pub number: [u8; HEX_COUNT],
    /// Intersections carrying each port kind.
    pub ports: [VertexSet; PORT_KINDS],

    // ---- Board, dynamic ----
    pub robber: u8,
    pub roads: [EdgeSet; MAX_PLAYERS],
    pub settlements: [VertexSet; MAX_PLAYERS],
    pub cities: [VertexSet; MAX_PLAYERS],

    // ---- Players ----
    pub hand: [[u8; 5]; MAX_PLAYERS],
    /// Development cards held but not played.
    pub dev_held: [[u8; 5]; MAX_PLAYERS],
    /// Of those, the ones bought this turn and so unplayable (R-9.4).
    pub dev_fresh: [[u8; 5]; MAX_PLAYERS],
    /// Militia played face up, which is what Largest Militia counts (R-10.8).
    pub militia_played: [u8; MAX_PLAYERS],
    pub roads_left: [u8; MAX_PLAYERS],
    pub settlements_left: [u8; MAX_PLAYERS],
    pub cities_left: [u8; MAX_PLAYERS],

    // ---- Shared ----
    pub supply: [u8; 5],
    pub dev_deck: [DevCard; DEV_DECK_SIZE],
    pub dev_drawn: u8,
    pub longest_road: Option<u8>,
    pub largest_militia: Option<u8>,

    // ---- Turn ----
    pub players: u8,
    pub to_act: u8,
    pub phase: Phase,
    pub dice: [u8; 2],
    /// One development card per turn (R-9.3).
    pub dev_played_this_turn: bool,
    /// Free roads still owed by a Road Building card (R-9.10).
    pub free_roads: u8,
    /// Cards each seat still owes to the discard (R-6.2).
    pub discard_left: [u8; MAX_PLAYERS],

    // ---- Trade market (R-7.19) ----
    pub trade_mode: TradeMode,
    /// What proposals are enumerated. See [`OfferShapes`].
    pub offer_shapes: OfferShapes,
    /// Proposals *generated* for one seat in one turn, at most (E-15).
    ///
    /// The same distinction as [`OfferShapes`]: a bound on enumeration, never
    /// on legality, which stays at [`OFFERS_PER_TURN`] (R-7.20). Time is the
    /// real cost of asking at a table, and this is time's deterministic
    /// proxy: past the allowance the engine offers a seat no further
    /// proposals this turn, so a policy choosing from generated candidates
    /// has to spend its asks where they count. Defaults to the rules cap,
    /// which makes it invisible until a table chooses otherwise.
    pub ask_allowance: u8,
    pub offers: [Offer; MAX_OFFERS],
    pub offer_count: u8,
    /// Offers each seat has made this turn, against the R-7.20 cap.
    pub offers_made: [u8; MAX_PLAYERS],
    /// The table's shared belief about every hand, from the public record
    /// alone (E-33). Engine-kept so replays rebuild it and search probes
    /// carry it forward; see [`crate::counting`].
    pub counting: crate::counting::Counting,

    pub rng: Rng,
}

impl State {
    /// A new game with the Random Setup board (R-3), before any placement.
    pub fn new(players: u8, seed: u64) -> Self {
        assert!(
            (3..=MAX_PLAYERS as u8).contains(&players),
            "Carranta is a 3–4 player game"
        );
        let mut rng = Rng::new(seed);

        // 19 terrain hexes in the fixed proportions of §3.1, placed at random
        // (R-3.2).
        let mut terrain = [Terrain::Desert; HEX_COUNT];
        {
            let mut bag: [Terrain; HEX_COUNT] = [
                Terrain::Hills,
                Terrain::Hills,
                Terrain::Hills,
                Terrain::Forest,
                Terrain::Forest,
                Terrain::Forest,
                Terrain::Forest,
                Terrain::Pasture,
                Terrain::Pasture,
                Terrain::Pasture,
                Terrain::Pasture,
                Terrain::Fields,
                Terrain::Fields,
                Terrain::Fields,
                Terrain::Fields,
                Terrain::Mountains,
                Terrain::Mountains,
                Terrain::Mountains,
                Terrain::Desert,
            ];
            rng.shuffle(Stream::Board, &mut bag);
            terrain.copy_from_slice(&bag);
        }

        // Number discs in letter order along the spiral, from a corner,
        // counterclockwise, skipping the desert (R-3.3).
        //
        // The sequence does the work the rules intend it to: the discs are
        // ordered so that following the path separates the high-probability
        // numbers. It is not a guarantee, though, because the desert can fall
        // anywhere and skipping it shifts everything after it, which is
        // exactly why R-3.12 exists. So the terrain and the starting corner
        // are drawn again when they land two reds together. Redrawing keeps
        // the result uniform over the boards that satisfy the constraint,
        // where nudging discs afterwards would bias which hex carries what.
        let mut number = [0u8; HEX_COUNT];
        for _ in 0..1_000 {
            let path = spiral_from(rng.below(Stream::Board, 6) as usize);
            let mut disc = 0;
            number = [0u8; HEX_COUNT];
            for h in path {
                if terrain[h as usize] != Terrain::Desert {
                    number[h as usize] = DISCS[disc];
                    disc += 1;
                }
            }
            debug_assert_eq!(disc, DISCS.len());
            if !red_numbers_touch(&number) {
                break;
            }
            // A fresh island as well: with the sequence fixed, where the
            // desert falls is most of what decides whether reds collide.
            let mut bag: [Terrain; HEX_COUNT] = terrain;
            rng.shuffle(Stream::Board, &mut bag);
            terrain = bag;
        }

        let robber = terrain
            .iter()
            .position(|t| *t == Terrain::Desert)
            .expect("the board has exactly one desert") as u8;

        let mut dev_deck = [DevCard::Militia; DEV_DECK_SIZE];
        {
            let mut i = 0;
            for (card, n) in DEV_DECK {
                for _ in 0..n {
                    dev_deck[i] = card;
                    i += 1;
                }
            }
            debug_assert_eq!(i, DEV_DECK_SIZE);
            rng.shuffle(Stream::DevDeck, &mut dev_deck);
        }

        State {
            terrain,
            number,
            ports: default_ports(),
            robber,
            roads: [0; MAX_PLAYERS],
            settlements: [0; MAX_PLAYERS],
            cities: [0; MAX_PLAYERS],
            hand: [[0; 5]; MAX_PLAYERS],
            dev_held: [[0; 5]; MAX_PLAYERS],
            dev_fresh: [[0; 5]; MAX_PLAYERS],
            militia_played: [0; MAX_PLAYERS],
            roads_left: [ROAD_POOL; MAX_PLAYERS],
            settlements_left: [SETTLEMENT_POOL; MAX_PLAYERS],
            cities_left: [CITY_POOL; MAX_PLAYERS],
            supply: [SUPPLY_PER_RESOURCE; 5],
            dev_deck,
            dev_drawn: 0,
            longest_road: None,
            largest_militia: None,
            players,
            to_act: 0,
            phase: Phase::SetupSettlement { round: 0 },
            dice: [0; 2],
            dev_played_this_turn: false,
            free_roads: 0,
            discard_left: [0; MAX_PLAYERS],
            trade_mode: TradeMode::default(),
            offer_shapes: OfferShapes::default(),
            ask_allowance: OFFERS_PER_TURN,
            offers: [Offer::default(); MAX_OFFERS],
            offer_count: 0,
            offers_made: [0; MAX_PLAYERS],
            counting: crate::counting::Counting::default(),
            rng,
        }
    }

    /// Turn player trading on for this game.
    pub fn with_trade_mode(mut self, mode: TradeMode) -> Self {
        self.trade_mode = mode;
        self
    }

    /// Choose what proposals the engine enumerates. See [`OfferShapes`].
    pub fn with_offer_shapes(mut self, shapes: OfferShapes) -> Self {
        self.offer_shapes = shapes;
        self
    }

    /// Cap the proposals generated per seat per turn (E-15). Clamped to the
    /// rules cap; zero generates none, and the composer is still free.
    pub fn with_ask_allowance(mut self, allowance: u8) -> Self {
        self.ask_allowance = allowance.min(OFFERS_PER_TURN);
        self
    }

    /// Live offers.
    #[inline]
    pub fn live_offers(&self) -> &[Offer] {
        &self.offers[..self.offer_count as usize]
    }

    /// Can `seat` hand over everything in `cards`?
    #[inline]
    pub fn holds(&self, seat: usize, cards: &[u8; 5]) -> bool {
        (0..5).all(|r| self.hand[seat][r] >= cards[r])
    }

    /// Every building a player owns.
    #[inline]
    pub fn buildings(&self, p: usize) -> VertexSet {
        self.settlements[p] | self.cities[p]
    }

    /// Every building on the board, whoever owns it.
    #[inline]
    pub fn all_buildings(&self) -> VertexSet {
        let mut v = 0;
        for p in 0..self.players as usize {
            v |= self.buildings(p);
        }
        v
    }

    /// Every road on the board.
    #[inline]
    pub fn all_roads(&self) -> EdgeSet {
        let mut e = 0;
        for p in 0..self.players as usize {
            e |= self.roads[p];
        }
        e
    }

    /// Intersections that break `p`'s routes: every opponent's buildings
    /// (R-10.3). A player's own never do.
    #[inline]
    pub fn blocking(&self, p: usize) -> VertexSet {
        let mut v = 0;
        for q in 0..self.players as usize {
            if q != p {
                v |= self.buildings(q);
            }
        }
        v
    }

    /// Do two states describe the same position, ignoring the generator?
    ///
    /// Replay supplies recorded randomness instead of drawing it (§7.1, H-1),
    /// so a replayed state's [`Rng`] has not advanced. The generator is engine
    /// bookkeeping and not part of the position, so it is excluded, while
    /// every other field, including any added later, is compared.
    pub fn same_game_as(&self, other: &State) -> bool {
        let mut a = *self;
        a.rng = other.rng;
        a == *other
    }

    /// Cards in hand, which is what the discard rule counts (R-6.2).
    /// Development cards are excluded (R-9.2).
    #[inline]
    pub fn hand_size(&self, p: usize) -> u32 {
        self.hand[p].iter().map(|&n| n as u32).sum()
    }

    /// Development cards held, playable or not.
    #[inline]
    pub fn dev_count(&self, p: usize) -> u32 {
        self.dev_held[p].iter().map(|&n| n as u32).sum()
    }

    /// Development cards of a kind that may be played this turn: held, less
    /// those bought this turn (R-9.4).
    #[inline]
    pub fn dev_playable(&self, p: usize, card: DevCard) -> u8 {
        self.dev_held[p][card as usize] - self.dev_fresh[p][card as usize]
    }

    /// Does `p` hold a building on a port of this kind?
    #[inline]
    pub fn has_port(&self, p: usize, kind: usize) -> bool {
        self.buildings(p) & self.ports[kind] != 0
    }

    /// The best rate at which `p` may trade `give` away (R-7.6 … R-7.9).
    #[inline]
    pub fn trade_rate(&self, p: usize, give: Resource) -> u8 {
        if self.has_port(p, give as usize + 1) {
            2
        } else if self.has_port(p, 0) {
            3
        } else {
            4
        }
    }

    /// Victory points, counting hidden Victory Point cards (R-11.3).
    ///
    /// This is the true total. What opponents can see is this less
    /// `dev_held[p][VictoryPoint]` (R-9.11).
    pub fn victory_points(&self, p: usize) -> u32 {
        let mut vp = self.settlements[p].count_ones() + 2 * self.cities[p].count_ones();
        if self.longest_road == Some(p as u8) {
            vp += 2;
        }
        if self.largest_militia == Some(p as u8) {
            vp += 2;
        }
        vp + self.dev_held[p][DevCard::VictoryPoint as usize] as u32
    }

    /// Victory points an opponent can see: hidden cards excluded.
    pub fn public_victory_points(&self, p: usize) -> u32 {
        self.victory_points(p) - self.dev_held[p][DevCard::VictoryPoint as usize] as u32
    }

    /// Intersections where `p` may legally place a settlement.
    ///
    /// The Distance Rule (R-8.5) bans any intersection within one edge of a
    /// building, so the forbidden set is every building plus its neighbours,
    /// computed here as a mask rather than a per-candidate walk.
    pub fn settlement_spots(&self, p: usize, setup: bool) -> VertexSet {
        let taken = self.all_buildings();
        let mut forbidden = taken;
        for v in crate::topology::iter_vertices(taken) {
            forbidden |= neighbors(v);
        }
        let mut spots = crate::topology::ALL_VERTICES & !forbidden;
        if !setup {
            // After setup a settlement must adjoin one of the player's own
            // roads (R-8.4).
            spots &= crate::topology::endpoints_of(self.roads[p]);
        }
        spots
    }

    /// Declare every card currently held to be publicly known (E-33).
    ///
    /// A test fixture's convenience: hands written directly into the struct
    /// bypassed [`crate::state::State::apply`], so the public record never
    /// saw them arrive. Real games never need this, cards arrive through
    /// actions and the record watches them do it.
    pub fn assume_hands_public(&mut self) {
        for p in 0..self.players as usize {
            for r in 0..5 {
                self.counting.expected[p][r] = self.hand[p][r] as u32 * crate::counting::SCALE;
            }
        }
    }

    /// Edges where `p` may legally build a road.
    ///
    /// A road must extend the player's own network (R-8.2) and may not reach
    /// past an opponent's building (R-8.3), so the junctions it may grow from
    /// are its own road ends and buildings, minus any intersection an opponent
    /// has built on.
    pub fn road_spots(&self, p: usize) -> EdgeSet {
        let occupied = self.all_roads();
        let mut from = crate::topology::endpoints_of(self.roads[p]) | self.buildings(p);
        from &= !self.blocking(p);
        let mut spots = 0;
        for v in crate::topology::iter_vertices(from) {
            spots |= edges_at(v);
        }
        spots & !occupied
    }

    /// Hexes producing on this roll, excluding the one the robber sits on
    /// (R-5.3, R-5.8).
    #[inline]
    pub fn producing_hexes(&self, roll: u8) -> u32 {
        let mut mask = 0u32;
        for h in 0..HEX_COUNT {
            if self.number[h] == roll && h as u8 != self.robber {
                mask |= 1 << h;
            }
        }
        mask
    }

    /// What each player is owed by a roll, before the supply is checked.
    ///
    /// A settlement earns one card, a city two (R-5.4, R-5.5).
    pub fn production(&self, roll: u8) -> [[u8; 5]; MAX_PLAYERS] {
        let mut owed = [[0u8; 5]; MAX_PLAYERS];
        let mut hexes = self.producing_hexes(roll);
        while hexes != 0 {
            let h = hexes.trailing_zeros() as u8;
            hexes &= hexes - 1;
            let Some(res) = self.terrain[h as usize].yields() else {
                continue;
            };
            let corners = hex_vertices(h);
            for (p, o) in owed.iter_mut().enumerate().take(self.players as usize) {
                let n = (self.settlements[p] & corners).count_ones()
                    + 2 * (self.cities[p] & corners).count_ones();
                o[res as usize] += n as u8;
            }
        }
        owed
    }
}

/// The number discs in the order they are laid down (R-3.3, ART-2).
///
/// The discs go face down in letter order and are placed along a fixed path,
/// so the *sequence* is what separates the high-probability numbers rather
/// than luck: 6s and 8s sit far apart in it. Dealing numbers at random
/// instead puts two reds together on about six boards in seven.
///
/// These eighteen values are content, not rules, the path in [`spiral_from`]
/// is what R-3.3 fixes, and a different sequence over the same multiset is a
/// design choice this is deliberately easy to change.
pub const DISCS: [u8; 18] = [5, 2, 6, 3, 8, 10, 9, 12, 11, 4, 8, 10, 9, 4, 5, 6, 3, 11];

/// The path the discs are laid along: from one corner, counterclockwise round
/// the coast, then the inner ring, then the middle (R-3.3).
///
/// `corner` selects which of the six corners to start from. The rules say
/// "any corner", and starting somewhere different is the only variety the
/// placement has once the sequence is fixed.
///
/// The result is a connected walk: every hex in it touches the one before.
pub fn spiral_from(corner: usize) -> [u8; HEX_COUNT] {
    // Axial ring: how many steps out from the middle a hex sits.
    let ring = |h: u8| {
        let [q, r] = crate::topology::hex_axial(h);
        let (q, r) = (q as i32, r as i32);
        (q.abs() + r.abs() + (q + r).abs()) as u32 / 2
    };
    // Screen position, so "counterclockwise" means what it does to a player
    // looking at the board rather than what it means to the lattice.
    let angle = |h: u8| {
        let [q, r] = crate::topology::hex_axial(h);
        let (x, y) = (3f64.sqrt() * (q as f64 + r as f64 / 2.0), 1.5 * r as f64);
        y.atan2(x)
    };

    let mut out = [0u8; HEX_COUNT];
    let mut next = 0;
    for r in (0..=2).rev() {
        let mut band: Vec<u8> = (0..HEX_COUNT as u8).filter(|&h| ring(h) == r).collect();
        if r == 0 {
            out[next] = band[0];
            next += 1;
            continue;
        }
        // The six corners of a ring are the hexes furthest from the middle;
        // on the inner ring every hex is one, which is why the start is taken
        // from the outer ring and the inner one simply follows round.
        let start = if r == 2 {
            let corners: Vec<u8> = {
                let mut c: Vec<u8> = band
                    .iter()
                    .copied()
                    .filter(|&h| {
                        let [q, rr] = crate::topology::hex_axial(h);
                        q == 0 || rr == 0 || q + rr == 0
                    })
                    .collect();
                c.sort_by(|&a, &b| angle(a).partial_cmp(&angle(b)).unwrap());
                c
            };
            corners[corner % corners.len()]
        } else {
            // Continue from wherever the last ring finished, so the walk stays
            // connected rather than jumping across the board.
            let prev = out[next - 1];
            *band
                .iter()
                .min_by(|&&a, &&b| {
                    let touching = |h: u8| {
                        if (hex_vertices(prev) & hex_vertices(h)).count_ones() == 2 {
                            0
                        } else {
                            1
                        }
                    };
                    (touching(a), angle(a) - angle(prev))
                        .partial_cmp(&(touching(b), angle(b) - angle(prev)))
                        .unwrap()
                })
                .expect("the inner ring is not empty")
        };

        // Counterclockwise on screen is decreasing angle, because y grows
        // downward in the drawing.
        let base = angle(start);
        band.sort_by(|&a, &b| {
            let key = |h: u8| {
                let mut d = base - angle(h);
                if d < -1e-9 {
                    d += std::f64::consts::TAU;
                }
                d
            };
            key(a).partial_cmp(&key(b)).unwrap()
        });
        for h in band {
            out[next] = h;
            next += 1;
        }
    }
    debug_assert_eq!(next, HEX_COUNT);
    out
}

/// Whether a 6 or an 8 sits next to another one (R-3.12, D-6).
///
/// The two highest-probability numbers are marked red on the disc for a
/// reason: putting them side by side concentrates a sixth of all production
/// on two touching hexes, and whoever opens on that corner is handed the
/// game. Two hexes are adjacent when they share an edge, which is to say two
/// intersections.
pub fn red_numbers_touch(number: &[u8; HEX_COUNT]) -> bool {
    let red = |n: u8| n == 6 || n == 8;
    (0..HEX_COUNT as u8).any(|a| {
        red(number[a as usize])
            && (a + 1..HEX_COUNT as u8).any(|b| {
                red(number[b as usize]) && (hex_vertices(a) & hex_vertices(b)).count_ones() == 2
            })
    })
}

/// The coastline, in order, as a closed walk.
///
/// A coastal intersection is one that does not touch three hexes, the sea
/// takes the place of the missing one. There are 30 of them and they form a
/// simple cycle, so the walk is unambiguous: every coastal vertex has exactly
/// two coastal neighbours, and taking the unvisited one each step goes round
/// the island once.
pub fn coast_ring() -> Vec<u8> {
    let mut touches = [0u8; VERTEX_COUNT];
    for h in 0..HEX_COUNT as u8 {
        for v in crate::topology::iter_vertices(hex_vertices(h)) {
            touches[v as usize] += 1;
        }
    }
    let coastal: VertexSet = (0..VERTEX_COUNT as u8)
        .filter(|&v| touches[v as usize] < 3)
        .fold(0, |m, v| m | vertex_bit(v));

    let start = coastal.trailing_zeros() as u8;
    let mut ring = vec![start];
    let mut seen = vertex_bit(start);
    while let Some(next) =
        crate::topology::iter_vertices(neighbors(*ring.last().unwrap()) & coastal & !seen).next()
    {
        ring.push(next);
        seen |= vertex_bit(next);
    }
    ring
}

/// The default port layout (ART-1).
///
/// Nine ports, four generic and one per resource. Each occupying **two
/// adjacent coastal intersections**, which is what a port is: a stretch of
/// coast with two landing points, either of which a building can claim
/// (R-7.9). A port on a single intersection would be half a port, and would
/// make port access rarer than the rules intend.
///
/// That accounts for 18 of the 30 coastal intersections. The remaining 12 are
/// spread as six single gaps and three double ones, so the ports go round the
/// whole island rather than crowding one shore. A generic port sits between
/// every pair of specific ones, so no two 2:1 ports are neighbours.
///
/// The *arrangement* is settled; which resource each 2:1 port serves relative
/// to the terrain under it is a content question that wants playtesting (§11).
fn default_ports() -> [VertexSet; PORT_KINDS] {
    // The layout never changes, and `State::new` runs millions of times in a
    // training session, so it is built once rather than walked per game.
    static PORTS: std::sync::OnceLock<[VertexSet; PORT_KINDS]> = std::sync::OnceLock::new();
    *PORTS.get_or_init(build_ports)
}

fn build_ports() -> [VertexSet; PORT_KINDS] {
    let ring = coast_ring();
    // Coastal intersections skipped after each port. Six ones and three twos:
    // 9 ports × 2 intersections + 12 skipped = the 30 the coast has, so the
    // pattern closes exactly on itself.
    //
    // The *order* matters as much as the counts, and is not obvious. Coastal
    // intersections are not evenly spaced by angle. The six corners of the
    // island bunch them up, so an even spacing counted in intersections is an
    // uneven one to look at. Starting `1, 2, 1` rather than `1, 1, 2` puts the
    // wide gaps where the coast is already turning a corner, which flattens
    // the spread from a 15° spacing range to 10°. That 10° is not merely
    // better, it is the best available: an exhaustive search over every way of
    // choosing 9 non-overlapping coastal edges finds nothing tighter, because
    // 9 ports do not divide evenly into a six-cornered coast.
    const GAPS: [usize; 9] = [1, 2, 1, 1, 2, 1, 1, 2, 1];
    // Index 0 is the generic 3:1; 1..=5 are the 2:1 ports, one per resource.
    const KINDS: [usize; 9] = [1, 0, 2, 0, 3, 0, 4, 0, 5];

    let mut ports = [0u64; PORT_KINDS];
    let mut at = 0;
    for (i, &kind) in KINDS.iter().enumerate() {
        ports[kind] |= vertex_bit(ring[at % ring.len()]) | vertex_bit(ring[(at + 1) % ring.len()]);
        at += 2 + GAPS[i];
    }
    ports
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_is_small_and_copy() {
        // The whole point of the representation: cloning a node is a memcpy of
        // a few cache lines (§6.3).
        fn assert_copy<T: Copy>() {}
        assert_copy::<State>();
        let size = core::mem::size_of::<State>();
        // 80 of these bytes are the public record (E-33), a belief a search
        // probe must carry through a copy, which is exactly why it lives
        // here and costs what it costs.
        assert!(size <= 576, "state grew to {size} bytes");
    }

    #[test]
    fn setup_board_has_the_right_composition() {
        for seed in 0..200 {
            let s = State::new(4, seed);
            let mut counts = [0; 6];
            for t in s.terrain {
                counts[t as usize] += 1;
            }
            assert_eq!(counts, [3, 4, 4, 4, 3, 1], "seed {seed}");

            // Exactly the desert lacks a number, and the discs are the right
            // multiset (R-3.3).
            let mut discs: Vec<u8> = s.number.iter().copied().filter(|&n| n != 0).collect();
            discs.sort_unstable();
            assert_eq!(
                discs,
                vec![2, 3, 3, 4, 4, 5, 5, 6, 6, 8, 8, 9, 9, 10, 10, 11, 11, 12]
            );
            assert_eq!(s.number[s.robber as usize], 0);
            assert_eq!(s.terrain[s.robber as usize], Terrain::Desert);
        }
    }

    #[test]
    fn dev_deck_has_the_right_composition() {
        let s = State::new(4, 1);
        let mut counts = [0u8; 5];
        for c in s.dev_deck {
            counts[c as usize] += 1;
        }
        assert_eq!(counts, [14, 5, 2, 2, 2]);
    }

    #[test]
    fn same_seed_gives_the_same_board() {
        assert!(State::new(4, 77) == State::new(4, 77));
        assert!(State::new(4, 77) != State::new(4, 78));
    }

    #[test]
    fn distance_rule_excludes_neighbours() {
        let mut s = State::new(4, 5);
        let v = 10u8;
        s.settlements[0] |= vertex_bit(v);
        let spots = s.settlement_spots(1, true);
        assert_eq!(spots & vertex_bit(v), 0, "the occupied spot is out");
        for e in crate::topology::iter_edges(edges_at(v)) {
            let w = crate::topology::edge_other(e, v);
            assert_eq!(spots & vertex_bit(w), 0, "neighbour {w} must be barred");
        }
        // Something two edges away is still available.
        assert!(spots != 0);
    }

    #[test]
    fn roads_may_not_pass_an_opponents_building() {
        let mut s = State::new(4, 6);
        // Player 0 has a road into intersection `v`; an opponent builds there.
        let v = 20u8;
        let e = crate::topology::iter_edges(edges_at(v)).next().unwrap();
        s.roads[0] |= crate::topology::edge_bit(e);
        let before = s.road_spots(0);
        let past: EdgeSet = edges_at(v) & !crate::topology::edge_bit(e);
        assert!(
            before & past != 0,
            "should reach past v before it is blocked"
        );

        s.settlements[1] |= vertex_bit(v);
        let after = s.road_spots(0);
        assert_eq!(after & past, 0, "an opponent's building blocks the way");
    }

    #[test]
    fn production_pays_two_for_a_city() {
        let mut s = State::new(4, 8);
        // Find a hex that produces something, and put a settlement and a city
        // of different players on it.
        let h = (0..HEX_COUNT)
            .find(|&h| s.terrain[h].yields().is_some() && h as u8 != s.robber)
            .unwrap();
        let roll = s.number[h];
        let corners: Vec<u8> = crate::topology::iter_vertices(hex_vertices(h as u8)).collect();
        s.settlements[0] |= vertex_bit(corners[0]);
        s.cities[1] |= vertex_bit(corners[2]);
        let res = s.terrain[h].yields().unwrap() as usize;

        let owed = s.production(roll);
        assert_eq!(owed[0][res], 1);
        assert_eq!(owed[1][res], 2);
    }

    #[test]
    fn the_robbers_hex_produces_nothing() {
        let mut s = State::new(4, 9);
        let h = (0..HEX_COUNT)
            .find(|&h| s.terrain[h].yields().is_some())
            .unwrap();
        let roll = s.number[h];
        let corners: Vec<u8> = crate::topology::iter_vertices(hex_vertices(h as u8)).collect();
        s.settlements[0] |= vertex_bit(corners[0]);
        let res = s.terrain[h].yields().unwrap() as usize;

        assert!(s.production(roll)[0][res] >= 1);
        s.robber = h as u8;
        assert_eq!(s.production(roll)[0][res], 0);
    }

    #[test]
    fn the_disc_path_is_one_connected_spiral_from_a_corner() {
        for corner in 0..6 {
            let path = spiral_from(corner);
            let mut seen = [false; HEX_COUNT];
            for &h in &path {
                assert!(!seen[h as usize], "hex {h} twice from corner {corner}");
                seen[h as usize] = true;
            }
            assert!(seen.iter().all(|&s| s), "corner {corner} misses a hex");

            // Every step lands on a hex touching the one before, which is what
            // makes it a walk round the board rather than an ordering that
            // merely happens to visit everything.
            for pair in path.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                assert_eq!(
                    (hex_vertices(a) & hex_vertices(b)).count_ones(),
                    2,
                    "corner {corner}: {a} does not touch {b}",
                );
            }
            // Outside first, middle last.
            let ring = |h: u8| {
                let [q, r] = crate::topology::hex_axial(h);
                let (q, r) = (q as i32, r as i32);
                (q.abs() + r.abs() + (q + r).abs()) / 2
            };
            let rings: Vec<i32> = path.iter().map(|&h| ring(h)).collect();
            assert_eq!(rings[..12], [2; 12], "the coast is walked first");
            assert_eq!(rings[12..18], [1; 6], "then the inner ring");
            assert_eq!(rings[18], 0, "the middle is last");
        }
    }

    #[test]
    fn the_six_starting_corners_give_six_different_boards() {
        // "Any corner" is the only variety left once the sequence is fixed,
        // so it had better actually vary.
        let paths: Vec<[u8; HEX_COUNT]> = (0..6).map(spiral_from).collect();
        for i in 0..6 {
            for j in i + 1..6 {
                assert_ne!(paths[i], paths[j], "corners {i} and {j} walk alike");
            }
        }
    }

    #[test]
    fn the_discs_are_the_set_the_rules_call_for() {
        let mut sorted = DISCS;
        sorted.sort_unstable();
        assert_eq!(
            sorted,
            [2, 3, 3, 4, 4, 5, 5, 6, 6, 8, 8, 9, 9, 10, 10, 11, 11, 12],
        );
    }

    #[test]
    fn no_board_puts_two_red_numbers_side_by_side() {
        // R-3.12 is on by default, so this holds for every board dealt, not
        // most of them. Unconstrained dealing violates it 86% of the time,
        // which is why it was so visible in play.
        for seed in 0..2_000 {
            let s = State::new(4, seed);
            assert!(
                !red_numbers_touch(&s.number),
                "seed {seed} deals adjacent red numbers: {:?}",
                s.number,
            );
        }
    }

    #[test]
    fn the_constraint_is_the_only_thing_the_redeal_changes() {
        // Dealing again must still deal a whole set of discs. A repair that
        // quietly dropped or duplicated one would pass the adjacency test.
        for seed in [1u64, 7, 99, 1234] {
            let s = State::new(4, seed);
            let mut dealt: Vec<u8> = s.number.iter().copied().filter(|&n| n > 0).collect();
            dealt.sort_unstable();
            assert_eq!(
                dealt,
                vec![2, 3, 3, 4, 4, 5, 5, 6, 6, 8, 8, 9, 9, 10, 10, 11, 11, 12],
                "seed {seed}",
            );
            assert_eq!(s.number[s.robber as usize], 0, "the desert has no disc");
        }
    }

    #[test]
    fn the_red_number_check_sees_adjacency_rather_than_index_distance() {
        // Hexes that are neighbours in the array are not always neighbours on
        // the board, and vice versa; a check on indices would be wrong in both
        // directions.
        let mut number = [3u8; HEX_COUNT];
        assert!(!red_numbers_touch(&number));

        // Two hexes sharing an edge share exactly two intersections.
        let (a, b) = (0..HEX_COUNT as u8)
            .flat_map(|a| (a + 1..HEX_COUNT as u8).map(move |b| (a, b)))
            .find(|&(a, b)| (hex_vertices(a) & hex_vertices(b)).count_ones() == 2)
            .expect("the board has adjacent hexes");
        number[a as usize] = 6;
        number[b as usize] = 8;
        assert!(red_numbers_touch(&number), "6 next to 8 is a violation");

        number[b as usize] = 3;
        let far = (0..HEX_COUNT as u8)
            .find(|&h| h != a && (hex_vertices(a) & hex_vertices(h)).count_ones() < 2)
            .expect("the board has non-adjacent hexes");
        number[far as usize] = 8;
        assert!(!red_numbers_touch(&number), "reds apart are fine");
    }

    #[test]
    fn the_coast_is_one_closed_walk_of_thirty_intersections() {
        let ring = coast_ring();
        assert_eq!(ring.len(), 30, "the island has 30 coastal intersections");
        let mut seen = 0u64;
        for (i, &v) in ring.iter().enumerate() {
            assert_eq!(seen & vertex_bit(v), 0, "vertex {v} appears twice");
            seen |= vertex_bit(v);
            let next = ring[(i + 1) % ring.len()];
            assert!(
                neighbors(v) & vertex_bit(next) != 0,
                "{v} and {next} are consecutive in the ring but not adjacent",
            );
        }
    }

    #[test]
    fn every_port_covers_two_adjacent_intersections() {
        // A port is a stretch of coast, not a point: a building on either of
        // its two landing places can use it (R-7.9).
        let s = State::new(4, 1);
        for kind in 0..PORT_KINDS {
            let vs: Vec<u8> = crate::topology::iter_vertices(s.ports[kind]).collect();
            let expected = if kind == 0 { 8 } else { 2 };
            assert_eq!(vs.len(), expected, "kind {kind} covers the wrong count");
        }
        // Each 2:1 port's two intersections are adjacent to each other.
        for kind in 1..PORT_KINDS {
            let vs: Vec<u8> = crate::topology::iter_vertices(s.ports[kind]).collect();
            assert!(
                neighbors(vs[0]) & vertex_bit(vs[1]) != 0,
                "kind {kind} is split across the coast rather than being one port",
            );
        }
    }

    #[test]
    fn ports_go_round_the_whole_island_without_touching() {
        let ring = coast_ring();
        let s = State::new(4, 1);
        let all: u64 = s.ports.iter().fold(0, |m, p| m | p);
        assert_eq!(
            all.count_ones(),
            18,
            "9 ports on two intersections each, none shared",
        );

        // Walk the coast and read off the runs: every port must be two long,
        // and every gap between ports one or two. A layout that clusters,
        // as the first one did, shows up here as a long portless stretch.
        let flags: Vec<bool> = ring.iter().map(|&v| all & vertex_bit(v) != 0).collect();
        let start = flags.iter().position(|&f| !f).expect("gaps exist");
        let mut runs: Vec<(bool, usize)> = Vec::new();
        for i in 0..flags.len() {
            let f = flags[(start + i) % flags.len()];
            match runs.last_mut() {
                Some((kind, n)) if *kind == f => *n += 1,
                _ => runs.push((f, 1)),
            }
        }
        let ports: Vec<usize> = runs.iter().filter(|(f, _)| *f).map(|(_, n)| *n).collect();
        let gaps: Vec<usize> = runs.iter().filter(|(f, _)| !*f).map(|(_, n)| *n).collect();
        assert_eq!(ports, vec![2; 9], "every port is two intersections wide");
        assert_eq!(gaps.len(), 9, "one gap between each pair of ports");
        assert!(
            gaps.iter().all(|&g| (1..=2).contains(&g)),
            "no stretch of coast is left without a port: {gaps:?}",
        );
    }

    #[test]
    fn the_ports_are_evenly_spread_round_the_island() {
        // Counting intersections is not enough: the coast bunches at the six
        // corners, so a layout can be evenly spaced along the shore and still
        // look lopsided. This measures what a player sees. The angle from the
        // middle of the board to each port.
        let ring = coast_ring();
        let s = State::new(4, 1);
        let all: u64 = s.ports.iter().fold(0, |m, p| m | p);

        let xy = |v: u8| {
            let t = crate::topology::vertex_axial(v);
            let q = t.iter().map(|p| p[0] as f64).sum::<f64>() / 3.0;
            let r = t.iter().map(|p| p[1] as f64).sum::<f64>() / 3.0;
            (3f64.sqrt() * (q + r / 2.0), 1.5 * r)
        };
        let mut angles: Vec<f64> = (0..30)
            .filter(|&i| {
                let (a, b) = (ring[i], ring[(i + 1) % 30]);
                all & vertex_bit(a) != 0 && all & vertex_bit(b) != 0
            })
            .map(|i| {
                let (ax, ay) = xy(ring[i]);
                let (bx, by) = xy(ring[(i + 1) % 30]);
                ((ay + by) / 2.0).atan2((ax + bx) / 2.0).to_degrees()
            })
            .collect();
        assert_eq!(angles.len(), 9, "nine ports, each on one coastal edge");
        angles.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let gaps: Vec<f64> = (0..9)
            .map(|i| {
                let mut d = angles[(i + 1) % 9] - angles[i];
                if d <= 0.0 {
                    d += 360.0;
                }
                d
            })
            .collect();
        let max = gaps.iter().cloned().fold(f64::MIN, f64::max);
        let min = gaps.iter().cloned().fold(f64::MAX, f64::min);
        // 10.2° is the tightest any legal layout achieves. An exhaustive
        // search over every choice of 9 non-overlapping coastal edges found
        // nothing better, since 9 ports do not divide a six-cornered coast
        // evenly. The first arrangement of these same gaps scored 14.8.
        assert!(
            max - min < 10.5,
            "ports are lopsided: gaps of {min:.0}°..{max:.0}° ({gaps:?})",
        );
    }

    #[test]
    fn no_intersection_serves_two_ports() {
        // Overlapping ports would hand one settlement two rates at once.
        let s = State::new(4, 1);
        let mut seen = 0u64;
        for p in s.ports {
            assert_eq!(seen & p, 0, "two ports share an intersection");
            seen |= p;
        }
    }

    #[test]
    fn trade_rate_follows_ports() {
        let mut s = State::new(4, 11);
        assert_eq!(s.trade_rate(0, Resource::Ore), 4, "no port is 4:1");

        let generic = crate::topology::iter_vertices(s.ports[0]).next().unwrap();
        s.settlements[0] |= vertex_bit(generic);
        assert_eq!(s.trade_rate(0, Resource::Ore), 3);

        let ore = crate::topology::iter_vertices(s.ports[Resource::Ore as usize + 1])
            .next()
            .unwrap();
        s.settlements[0] |= vertex_bit(ore);
        assert_eq!(s.trade_rate(0, Resource::Ore), 2);
        assert_eq!(s.trade_rate(0, Resource::Wood), 3, "the 2:1 is ore only");
    }

    #[test]
    fn victory_point_cards_are_hidden_from_opponents() {
        let mut s = State::new(4, 12);
        s.settlements[0] |= vertex_bit(3);
        s.dev_held[0][DevCard::VictoryPoint as usize] = 2;
        assert_eq!(s.victory_points(0), 3);
        assert_eq!(s.public_victory_points(0), 1);
    }
}
