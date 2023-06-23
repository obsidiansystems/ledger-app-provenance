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
    Input {
        , address : (LengthDelimitedParser, String, false) = 1
        , coins : (LengthDelimitedParser, super::super::super::cosmos::base::v1beta1::Coin, true) = 2
    }
}

define_message! { @impl
    Output {
        , address : (LengthDelimitedParser, String, false) = 1
        , coins : (LengthDelimitedParser, super::super::super::cosmos::base::v1beta1::Coin, true) = 2
    }
}

define_message! { @impl
    MsgSend {
        , from_address : (LengthDelimitedParser, String, false) = 1
        , to_address : (LengthDelimitedParser, String, false) = 2
        , amount : (LengthDelimitedParser, super::super::super::cosmos::base::v1beta1::Coin, true) = 3
    }
}

define_message! { @impl
    MsgMultiSend {
        , inputs : (LengthDelimitedParser, super::super::super::cosmos::bank::v1beta1::Input, true) = 1
        , outputs : (LengthDelimitedParser, super::super::super::cosmos::bank::v1beta1::Output, true) = 2
    }
}
