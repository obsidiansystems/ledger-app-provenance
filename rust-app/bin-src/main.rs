#![allow(incomplete_features)]
#![feature(const_eval_limit)]
#![const_eval_limit = "0"]
#![cfg_attr(target_family = "bolos", no_std)]
#![cfg_attr(target_family = "bolos", no_main)]

#[cfg(not(target_family = "bolos"))]
fn main() {}

#[cfg(target_family = "bolos")]
mod main_nanos;
#[cfg(target_family = "bolos")]
pub use main_nanos::*;
