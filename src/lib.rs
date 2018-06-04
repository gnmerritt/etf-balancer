#[macro_use]
extern crate serde_derive;

#[cfg(test)]
extern crate spectral;

pub mod accounts;
pub use accounts::balancer::run_balancing;
