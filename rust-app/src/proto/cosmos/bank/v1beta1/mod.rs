#[allow(unused_imports)]
use ledger_parser_combinators::{define_message, define_enum, interp_parser::DefaultInterp, async_parser::{HasOutput, AsyncParser, Readable, LengthDelimitedParser, reject,reject_on}, protobufs::{schema::*, async_parser::*}};
#[allow(unused_imports)]
use ledger_log::*;
#[allow(unused_imports)]
use core::future::Future;

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

