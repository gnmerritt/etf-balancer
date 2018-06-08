#[macro_use]
extern crate serde_derive;

#[macro_use(c)]
extern crate cute;

extern crate stats;

#[cfg(test)]
extern crate spectral;

pub mod accounts;
pub use accounts::balancer::run_balancing;
