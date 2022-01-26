use ledger_parser_combinators::core_parsers::*;
use ledger_parser_combinators::define_json_struct;
use ledger_parser_combinators::endianness::*;
use ledger_parser_combinators::json::*;

// Payload for a public key request
pub type Bip32Key = DArray<Byte, U32<{ Endianness::Little }>, 10>;

define_json_struct! { Meta 16 {
    chainId: JsonString,
    sender: JsonString,
    gasLimit: JsonNumber,
    gasPrice: JsonNumber,
    ttl: JsonNumber,
    creationTime: JsonNumber
}}

define_json_struct! { Signer 16 {
    scheme: JsonString,
    pubKey: JsonString,
    addr: JsonString,
    caps: JsonArray<JsonString>
}}

// This should just be called Amount, but we have a name collition between
// field names and type names
define_json_struct! { AmountType 16 {
  amount: JsonString,
  denom: JsonString
}}

define_json_struct! { Fee 16 {
  amount: JsonArray<AmountTypeSchema>,
  gas: JsonString
}}

define_json_struct! { SendValue 16 {
  amount: JsonString,
  from_address: JsonString,
  to_address: JsonString
}}

define_json_struct! { UnjailValue 16 {
  address: JsonString
}}

define_json_struct! { PublicKey 16 {
  type: JsonString,
  value: JsonString
}}

define_json_struct! { StakeValue 16 {
  chains: JsonArray<JsonString>,
  public_key: PublicKeySchema,
  service_url: JsonString,
  value: JsonString
}}

define_json_struct! { UnstakeValue 16 {
  validator_address: JsonString
}}

pub struct MessageSchema;

define_json_struct! { ProvenanceCmd 16 {
  chain_id: JsonString,
  entropy: JsonString,
  fee: JsonArray<AmountTypeSchema>,
  memo: JsonString,
  msgs: JsonArray<MessageSchema>
}}

// Payload for a signature request, content-agnostic.
pub type SignParameters = (
    LengthFallback<U32<{ Endianness::Little }>, Json<ProvenanceCmdSchema>>,
    Bip32Key,
);
