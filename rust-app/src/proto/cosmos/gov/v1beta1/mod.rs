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
    MsgDeposit {
        , proposal_id : (AsyncParser, Uint64, false) = 1
        , depositor : (LengthDelimitedParser, String, false) = 2
        , amount : (LengthDelimitedParser, super::super::super::cosmos::base::v1beta1::Coin, true) = 3
    }
}
