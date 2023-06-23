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
    Tx {
        , body : (LengthDelimitedParser, super::super::super::cosmos::tx::v1beta1::TxBody, false) = 1
        , auth_info : (LengthDelimitedParser, super::super::super::cosmos::tx::v1beta1::AuthInfo, false) = 2
        , signatures : (LengthDelimitedParser, Bytes, true) = 3
    }
}

define_message! { @impl
    SignDoc {
        , body_bytes : (LengthDelimitedParser, Bytes, false) = 1
        , auth_info_bytes : (LengthDelimitedParser, Bytes, false) = 2
        , chain_id : (LengthDelimitedParser, String, false) = 3
        , account_number : (AsyncParser, Uint64, false) = 4
    }
}

define_message! { @impl
    TxBody {
        , messages : (LengthDelimitedParser, super::super::super::google::protobuf::Any, true) = 1
        , memo : (LengthDelimitedParser, String, false) = 2
        , timeout_height : (AsyncParser, Uint64, false) = 3
        , extension_options : (LengthDelimitedParser, super::super::super::google::protobuf::Any, true) = 1023
        , non_critical_extension_options : (LengthDelimitedParser, super::super::super::google::protobuf::Any, true) = 2047
    }
}

define_message! { @impl
    AuthInfo {
        , signer_infos : (LengthDelimitedParser, super::super::super::cosmos::tx::v1beta1::SignerInfo, true) = 1
        , fee : (LengthDelimitedParser, super::super::super::cosmos::tx::v1beta1::Fee, false) = 2
        , tip : (LengthDelimitedParser, super::super::super::cosmos::tx::v1beta1::Tip, false) = 3
    }
}

define_message! { @impl
    SignerInfo {
        , public_key : (LengthDelimitedParser, super::super::super::google::protobuf::Any, false) = 1
        , mode_info : (LengthDelimitedParser, super::super::super::cosmos::tx::v1beta1::ModeInfo, false) = 2
        , sequence : (AsyncParser, Uint64, false) = 3
    }
}

pub mod modeinfo;

define_message! { @impl
    ModeInfo {
        , single : (LengthDelimitedParser, super::super::super::cosmos::tx::v1beta1::modeinfo::Single, false) = 1
        , multi : (LengthDelimitedParser, super::super::super::cosmos::tx::v1beta1::modeinfo::Multi, false) = 2
    }
}

define_message! { @impl
    Fee {
        , amount : (LengthDelimitedParser, super::super::super::cosmos::base::v1beta1::Coin, true) = 1
        , gas_limit : (AsyncParser, Uint64, false) = 2
        , payer : (LengthDelimitedParser, String, false) = 3
        , granter : (LengthDelimitedParser, String, false) = 4
    }
}

define_message! { @impl
    Tip {
        , amount : (LengthDelimitedParser, super::super::super::cosmos::base::v1beta1::Coin, true) = 1
        , tipper : (LengthDelimitedParser, String, false) = 2
    }
}
