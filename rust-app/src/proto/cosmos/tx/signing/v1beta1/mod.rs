#[allow(unused_imports)]
use ledger_parser_combinators::{define_message, define_enum, interp_parser::DefaultInterp, async_parser::{HasOutput, AsyncParser, Readable, LengthDelimitedParser, reject,reject_on}, protobufs::{schema::*, async_parser::*}};
#[allow(unused_imports)]
use ledger_log::*;
#[allow(unused_imports)]
use core::future::Future;

define_enum! {
    SignMode {
        SIGN_MODE_UNSPECIFIED = 0,
        SIGN_MODE_DIRECT = 1,
        SIGN_MODE_TEXTUAL = 2,
        SIGN_MODE_DIRECT_AUX = 3,
        SIGN_MODE_LEGACY_AMINO_JSON = 127,
        SIGN_MODE_EIP_191 = 191
    }
}

