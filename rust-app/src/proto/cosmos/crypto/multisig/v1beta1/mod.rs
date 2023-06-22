#[allow(unused_imports)]
use ledger_parser_combinators::{define_message, define_enum, interp_parser::DefaultInterp, async_parser::{HasOutput, AsyncParser, Readable, LengthDelimitedParser, reject,reject_on}, protobufs::{schema::*, async_parser::*}};
#[allow(unused_imports)]
use ledger_log::*;
#[allow(unused_imports)]
use core::future::Future;

define_message! { @impl
    CompactBitArray {
        , extra_bits_stored : (AsyncParser, Uint32, false) = 1
        , elems : (LengthDelimitedParser, Bytes, false) = 2
    }
}

