use ledger_parser_combinators::core_parsers::*;
// use ledger_parser_combinators::protobufs::schema::*;
use ledger_parser_combinators::endianness::*;

pub use crate::proto::cosmos::tx::v1beta1::SignDoc;
pub use crate::proto::cosmos::bank::v1beta1::MsgSend;
pub use crate::proto::google::protobuf::Any;

// Payload for a public key request
pub type Bip32Key = DArray<Byte, U32<{ Endianness::Little }>, 10>;

// Payload for a signature request, content-agnostic.
pub type Transaction = SignDoc;

#[repr(u8)]
#[derive(Debug)]
pub enum Ins {
    GetVersion,
    GetPubkey,
    Sign,
    GetVersionStr,
    Exit
}

impl From<u8> for Ins {
    fn from(ins: u8) -> Ins {
        match ins {
            0 => Ins::GetVersion,
            2 => Ins::GetPubkey,
            3 => Ins::Sign,
            0xfe => Ins::GetVersionStr,
            0xff => Ins::Exit,
            _ => panic!(),
        }
    }
}
