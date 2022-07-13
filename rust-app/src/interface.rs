use ledger_parser_combinators::core_parsers::*;
use ledger_parser_combinators::define_json_struct;
use ledger_parser_combinators::endianness::*;
use ledger_parser_combinators::json::*;

// Payload for a public key request
pub type Bip32Key = DArray<Byte, U32<{ Endianness::Little }>, 10>;

// This should just be called Amount, but we have a name collition between
// field names and type names
define_json_struct! { AmountType 16 {
  amount: JsonString,
  denom: JsonString
}}

define_json_struct! { SendValue 16 {
  fromAddress: JsonString,
  toAddress: JsonString,
  amount: AmountTypeSchema
}}

define_json_struct! { DelegateValue 16 {
  delegatorAddress: JsonString,
  validatorAddress: JsonString,
  amount: AmountTypeSchema
}}

define_json_struct! { UndelegateValue 16 {
  delegatorAddress: JsonString,
  validatorAddress: JsonString,
  amount: AmountTypeSchema
}}

pub struct MessageSchema;

define_json_struct! { ProvenanceCmd 16 {
  messages: JsonArray<MessageSchema>
}}

// Payload for a signature request, content-agnostic.
pub type SignParameters = (
    LengthFallback<U32<{ Endianness::Little }>, Json<ProvenanceCmdSchema>>,
    Bip32Key,
);
