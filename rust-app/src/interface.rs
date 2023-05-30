use core::convert::TryFrom;
use ledger_parser_combinators::core_parsers::*;
// use ledger_parser_combinators::protobufs::schema::*;
use ledger_parser_combinators::endianness::*;
use nanos_sdk::io::ApduHeader;
use num_enum::TryFromPrimitive;

pub use crate::proto::cosmos::bank::v1beta1::MsgSend;
pub use crate::proto::cosmos::tx::v1beta1::SignDoc;
pub use crate::proto::google::protobuf::Any;

// Payload for a public key request
pub type Bip32Key = DArray<Byte, U32<{ Endianness::Little }>, 10>;

// Payload for a signature request, content-agnostic.
pub type Transaction = SignDoc;

#[repr(u8)]
#[derive(Debug, TryFromPrimitive)]
pub enum Ins {
    GetVersion = 0,
    VerifyAddress = 1,
    GetPubkey = 2,
    Sign = 3,
    GetVersionStr = 0xfe,
    Exit = 0xff,
}

impl TryFrom<ApduHeader> for Ins {
    type Error = ();
    fn try_from(m: ApduHeader) -> Result<Ins, Self::Error> {
        match m {
            ApduHeader {
                cla: 0,
                ins,
                p1: 0,
                p2: 0,
            } => Self::try_from(ins).map_err(|_| ()),
            _ => Err(()),
        }
    }
}
