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

define_message! { @impl
    Coin {
        , denom : (LengthDelimitedParser, String, false) = 1
        , amount : (LengthDelimitedParser, String, false) = 2
    }
}
