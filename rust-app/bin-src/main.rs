#![allow(incomplete_features)]
#![feature(const_eval_limit)]
#![feature(impl_trait_in_bindings)]
#![feature(maybe_uninit_extra)]
#![feature(maybe_uninit_ref)]
#![feature(min_type_alias_impl_trait)]
#![const_eval_limit = "0"]
#![cfg_attr(target_os = "bolos", no_std)]
#![cfg_attr(target_os = "bolos", no_main)]

#[cfg(not(target_family = "bolos"))]
fn main() {}

#[cfg(target_family = "bolos")]
mod main_nanos;
#[cfg(target_family = "bolos")]
pub use main_nanos::*;
