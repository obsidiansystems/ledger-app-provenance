#[allow(unused_imports)]
use ledger_parser_combinators::{define_message, define_enum, interp_parser::DefaultInterp, async_parser::{HasOutput, AsyncParser, Readable, LengthDelimitedParser, reject,reject_on}, protobufs::{schema::*, async_parser::*}};
#[allow(unused_imports)]
use ledger_log::*;
#[allow(unused_imports)]
use core::future::Future;

define_message! { @impl
    MsgDeposit {
        , proposal_id : (AsyncParser, Uint64, false) = 1
        , depositor : (LengthDelimitedParser, String, false) = 2
        , amount : (LengthDelimitedParser, super::super::super::cosmos::base::v1beta1::Coin, true) = 3
    }
}

