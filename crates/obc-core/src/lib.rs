//! Fixed-point math, angles, random numbers and constants shared by the
//! OBCR engine (DESIGN §6.5). Nothing here uses floating point at run time.

pub mod fixed;
pub mod input;
pub mod rng;

/// Simulation tics per second (`TICRATE`).
pub const TICRATE: u32 = 70;
/// Map side in tiles (`MAPSIZE`).
pub const MAPSIZE: usize = 64;
