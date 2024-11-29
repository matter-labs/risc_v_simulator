#![allow(incomplete_features)]
#![feature(generic_const_exprs)]
#![feature(array_chunks)]
#![feature(iter_array_chunks)]
#![feature(let_chains)]

pub mod abstractions;
pub mod cycle;
pub mod mmio;
pub mod mmu;
mod qol;
pub mod runner;
pub mod sim;
pub mod utils;

#[cfg(feature = "delegation")]
pub mod delegations;

#[cfg(test)]
mod tests;
