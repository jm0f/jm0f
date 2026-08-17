//! Play Carranta locally in a browser.
//!
//! One process, no dependencies, no build step: `carranta-play` binds a port on
//! the loopback address and serves a single page that talks to the engine over
//! three JSON endpoints.
//!
//! The page is served **the redacted view**, never the state. Everything it
//! receives goes through the §7.3 projection, so it cannot be sent another
//! seat's cards or the deck order, the type it is built from has no field for
//! them. Doing that here rather than later matters: a local UI that read the
//! raw state would teach the codebase a habit the real server then has to
//! unpick.

pub mod analysis;
pub mod game;
pub mod home;
pub mod json;
pub mod report;
pub mod server;
pub mod store;
pub mod view;

pub use game::{Choice, Session, Target};
pub use server::Server;
