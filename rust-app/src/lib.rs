#![no_std]
#![allow(incomplete_features)]
#![feature(const_mut_refs)]
#![feature(future_poll_fn)]
#![feature(generic_associated_types)]
#![feature(rustc_attrs)]
#![feature(str_internals)]
#![feature(type_alias_impl_trait)]
#![cfg_attr(all(target_family = "bolos", test), no_main)]
#![cfg_attr(target_family = "bolos", feature(custom_test_frameworks))]
#![reexport_test_harness_main = "test_main"]
#![cfg_attr(target_family = "bolos", test_runner(nanos_sdk::sdk_test_runner))]
#![feature(const_eval_limit)]
#![const_eval_limit = "0"]

#[macro_use]
extern crate num_derive;

mod proto {
    include!(concat!(env!("OUT_DIR"), "/proto/mod.rs"));
}

pub use ledger_log::*;

#[cfg(all(target_family = "bolos", test))]
#[no_mangle]
extern "C" fn sample_main() {
    use nanos_sdk::exit_app;
    test_main();
    exit_app(0);
}

pub mod interface;

#[cfg(all(target_family = "bolos"))]
pub mod crypto_helpers;

#[cfg(all(target_family = "bolos"))]
pub mod implementation;

#[cfg(all(target_family = "bolos"))]
pub mod trampolines;

#[cfg(all(target_family = "bolos", test))]
use core::panic::PanicInfo;
/// In case of runtime problems, return an internal error and exit the app
#[cfg(all(target_family = "bolos", test))]
#[inline]
#[cfg_attr(all(target_family = "bolos", test), panic_handler)]
pub fn exiting_panic(_info: &PanicInfo) -> ! {
    //let mut comm = io::Comm::new();
    //comm.reply(io::StatusWords::Panic);
    error!("Panicking: {:?}\n", _info);
    nanos_sdk::exit_app(1)
}

///// Custom type used to implement tests
//#[cfg(all(target_os = "nanos", test))]
//use nanos_sdk::TestType;
