#[allow(unused_imports)]
use ledger_parser_combinators::{define_message, define_enum, interp_parser::DefaultInterp, async_parser::{HasOutput, AsyncParser, Readable, LengthDelimitedParser, reject,reject_on}, protobufs::{schema::*, async_parser::*}};
#[allow(unused_imports)]
use ledger_log::*;
#[allow(unused_imports)]
use core::future::Future;

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

