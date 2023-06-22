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
    MsgDelegate {
        , delegator_address : (LengthDelimitedParser, String, false) = 1
        , validator_address : (LengthDelimitedParser, String, false) = 2
        , amount : (LengthDelimitedParser, super::super::super::cosmos::base::v1beta1::Coin, false) = 3
    }
}

define_message! { @impl
    MsgBeginRedelegate {
        , delegator_address : (LengthDelimitedParser, String, false) = 1
        , validator_src_address : (LengthDelimitedParser, String, false) = 2
        , validator_dst_address : (LengthDelimitedParser, String, false) = 3
        , amount : (LengthDelimitedParser, super::super::super::cosmos::base::v1beta1::Coin, false) = 4
    }
}

define_message! { @impl
    MsgUndelegate {
        , delegator_address : (LengthDelimitedParser, String, false) = 1
        , validator_address : (LengthDelimitedParser, String, false) = 2
        , amount : (LengthDelimitedParser, super::super::super::cosmos::base::v1beta1::Coin, false) = 3
    }
}
