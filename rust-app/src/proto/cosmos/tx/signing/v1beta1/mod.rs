#[allow(unused_imports)]
use core::future::Future;
#[allow(unused_imports)]
use ledger_log::*;
#[allow(unused_imports)]
use ledger_parser_combinators::{
    async_parser::{reject, reject_on, AsyncParser, HasOutput, LengthDelimitedParser, Readable},
    define_enum, define_message,
    interp_parser::DefaultInterp,
    protobufs::{async_parser::*, schema::*},
};

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
