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
    Single {
        , mode : (AsyncParser, super::super::super::super::cosmos::tx::signing::v1beta1::SignMode, false) = 1
    }
}

define_message! { @impl
    Multi {
        , bitarray : (LengthDelimitedParser, super::super::super::super::cosmos::crypto::multisig::v1beta1::CompactBitArray, false) = 1
        , mode_infos : (LengthDelimitedParser, super::super::super::super::cosmos::tx::v1beta1::ModeInfo, true) = 2
    }
}
