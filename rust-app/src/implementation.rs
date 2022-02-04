use crate::crypto_helpers::{detecdsa_sign, get_pkh, get_private_key, get_pubkey, Hasher};
use crate::interface::*;
use crate::*;
use arrayvec::{ArrayString, ArrayVec};
use core::fmt::Write;
use core::fmt::Debug;
use ledger_parser_combinators::interp_parser::{
    Action, DefaultInterp, DropInterp, InterpParser, ObserveLengthedBytes, SubInterp, OOB, set_from_thunk
};
use ledger_parser_combinators::json::Json;
use nanos_ui::ui;
use prompts_ui::{write_scroller, final_accept_prompt};
use core::str::from_utf8;
use core::convert::TryFrom;

use ledger_parser_combinators::define_json_struct_interp;
use ledger_parser_combinators::json::*;
use ledger_parser_combinators::json_interp::*;

// A couple type ascription functions to help the compiler along.
const fn mkfn<A,B,C>(q: fn(&A,&mut B)->C) -> fn(&A,&mut B)->C {
  q
}
const fn mkvfn<A,C>(q: fn(&A,&mut Option<()>)->C) -> fn(&A,&mut Option<()>)->C {
  q
}

pub type GetAddressImplT = impl InterpParser<Bip32Key, Returning = ArrayVec<u8, 128>>;

pub const GET_ADDRESS_IMPL: GetAddressImplT =
    Action(SubInterp(DefaultInterp), mkfn(|path: &ArrayVec<u32, 10>, destination: &mut Option<ArrayVec<u8, 128>>| -> Option<()> {
        let key = get_pubkey(path).ok()?;

        let pkh = get_pkh(key).ok()?;

        write_scroller("Provide Public Key", |w| Ok(write!(w, "For Address     {}", pkh)?))?;

        final_accept_prompt(&[])?;

        let rv = destination.insert(ArrayVec::new());
        rv.try_push(u8::try_from(key.len()).ok()?).ok()?;
        rv.try_extend_from_slice(&key).ok()?;
        Some(())
    }));

const FROM_ADDRESS_ACTION: impl JsonInterp<JsonString, State: Debug> =
  Action(JsonStringAccumulate::<64>,
        mkvfn(| from_address: &ArrayVec<u8, 64>, destination | {
          write_scroller("Transfer from", |w| Ok(write!(w, "{}", from_utf8(from_address.as_slice())?)?))?;
          *destination = Some(());
          Some(())
        }));

const TO_ADDRESS_ACTION: impl JsonInterp<JsonString, State: Debug> =
  Action(JsonStringAccumulate::<64>,
        mkvfn(| to_address: &ArrayVec<u8, 64>, destination | {
            write_scroller("Transfer To", |w| Ok(write!(w, "{}", from_utf8(to_address.as_slice())?)?))?;
            *destination = Some(());
            Some(())
        }));

const AMOUNT_ACTION: impl JsonInterp<AmountTypeSchema, State: Debug> =
  Action(AmountTypeInterp { field_amount: JsonStringAccumulate::<64>, field_denom: JsonStringAccumulate::<64> },
        mkvfn(| &AmountType { field_amount: ref amount, field_denom: ref denom }: &AmountType<Option<ArrayVec<u8, 64>>, Option<ArrayVec<u8, 64>>>,
                destination | {
          write_scroller("Amount:", |w| Ok(write!(w, "{} ({})", from_utf8(amount.as_ref()?)?, from_utf8(denom.as_ref()?)?)?))?;
          *destination = Some(());
          Some(())
        }));

const SEND_MESSAGE_ACTION: impl JsonInterp<SendValueSchema, State: Debug> =
  Preaction(|| { write_scroller("Send", |w| Ok(write!(w, "Transaction")?)) },
  SendValueInterp{field_amount: AMOUNT_ACTION,
            field_from_address: FROM_ADDRESS_ACTION,
            field_to_address: TO_ADDRESS_ACTION});

const DELEGATOR_ADDRESS_ACTION: impl JsonInterp<JsonString, State: Debug> =
  Action(JsonStringAccumulate::<64>,
        mkvfn(| delegator_address: &ArrayVec<u8, 64>, destination | {
          write_scroller("Delegator", |w| Ok(write!(w, "{}", from_utf8(delegator_address.as_slice())?)?))?;
          *destination = Some(());
          Some(())
        }));

const VALIDATOR_ADDRESS_ACTION: impl JsonInterp<JsonString, State: Debug> =
  Action(JsonStringAccumulate::<64>,
        mkvfn(| validator_address: &ArrayVec<u8, 64>, destination | {
            write_scroller("Validator", |w| Ok(write!(w, "{}", from_utf8(validator_address.as_slice())?)?))?;
            *destination = Some(());
            Some(())
        }));

const DELEGATE_MESSAGE_ACTION: impl JsonInterp<DelegateValueSchema, State: Debug> =
  Preaction(|| { write_scroller("Delegate", |w| Ok(write!(w, "Transaction")?)) }, DelegateValueInterp{
    field_delegator_address: DELEGATOR_ADDRESS_ACTION,
    field_validator_address: VALIDATOR_ADDRESS_ACTION,
    field_amount: AMOUNT_ACTION});

const UNDELEGATE_MESSAGE_ACTION: impl JsonInterp<UndelegateValueSchema, State: Debug> =
  Preaction(|| { write_scroller("Undelegate", |w| Ok(write!(w, "Transaction")?)) }, UndelegateValueInterp{
    field_delegator_address: DELEGATOR_ADDRESS_ACTION,
    field_validator_address: VALIDATOR_ADDRESS_ACTION,
    field_amount: AMOUNT_ACTION});

pub type SignImplT = impl InterpParser<SignParameters, Returning = ArrayVec<u8, 128>>;

pub const SIGN_IMPL: SignImplT = Action(
    (
        Action(
            // Calculate the hash of the transaction
            ObserveLengthedBytes(
                Hasher::new,
                Hasher::update,
                Json(ProvenanceCmdInterp {
                    field_messages: SubInterp(Message {
                        send_message: SEND_MESSAGE_ACTION,
                        delegate_message: DELEGATE_MESSAGE_ACTION,
                        undelegate_message: UNDELEGATE_MESSAGE_ACTION,
                    }),
                }),
                true,
            ),
            // Ask the user if they accept the transaction body's hash
            mkfn(|(_, hash): &(_, Hasher), destination: &mut Option<[u8; 32]>| {
                let the_hash = hash.clone().finalize();
                write_scroller("Sign Hash?", |w| Ok(write!(w, "{}", the_hash)?))?;
                *destination = Some(the_hash.0.into());
                Some(())
            }),
        ),
        Action(
            SubInterp(DefaultInterp),
            // And ask the user if this is the key the meant to sign with:
            mkfn(|path: &ArrayVec<u32, 10>, destination| {
                let privkey = get_private_key(path).ok()?;
                let pubkey = get_pubkey(path).ok()?; // Redoing work here; fix.
                let pkh = get_pkh(pubkey).ok()?;

                write_scroller("For Account", |w| Ok(write!(w, "{}", pkh)?))?;

                *destination = Some(privkey);
                Some(())
            }),
        ),
    ),
    mkfn(|(hash, key): &(Option<[u8; 32]>, Option<_>), destination: &mut Option<ArrayVec<u8, 128>>| {
        // By the time we get here, we've approved and just need to do the signature.
        final_accept_prompt(&[])?;
        let sig = detecdsa_sign(hash.as_ref()?, key.as_ref()?)?;
        let rv = destination.insert(ArrayVec::new());
        rv.try_extend_from_slice(&sig).ok()?;
        Some(())
    }),
);

// The global parser state enum; any parser above that'll be used as the implementation for an APDU
// must have a field here.

pub enum ParsersState {
    NoState,
    GetAddressState(<GetAddressImplT as InterpParser<Bip32Key>>::State),
    SignState(<SignImplT as InterpParser<SignParameters>>::State),
}

pub fn reset_parsers_state(state: &mut ParsersState) {
    *state = ParsersState::NoState;
}

amount_type_definition!{}
send_value_definition!{}
delegate_value_definition!{}
undelegate_value_definition!{}

#[derive(Copy, Clone, Debug)]
pub enum MessageType {
  SendMessage,
  DelegateMessage,
  UndelegateMessage,
}

#[derive(Debug)]
pub struct Message<
  SendInterp: JsonInterp<SendValueSchema>,
  DelegateInterp: JsonInterp<DelegateValueSchema>,
  UndelegateInterp: JsonInterp<UndelegateValueSchema>> {
  pub send_message: SendInterp,
  pub delegate_message: DelegateInterp,
  pub undelegate_message: UndelegateInterp
}

type TemporaryStringState<const N: usize>  = <JsonStringAccumulate<N> as JsonInterp<JsonString>>::State;
type TemporaryStringReturn<const N: usize> = Option<<JsonStringAccumulate<N> as JsonInterp<JsonString>>::Returning>;


const TYPE_LEN: usize = 5;

const TYPE: [u8; 5] = *b"@type";

#[derive(Debug)]
pub enum MessageState<SendMessageState, DelegateMessageState, UndelegateMessageState> {
  Start,
  TypeLabel(TemporaryStringState<TYPE_LEN>, TemporaryStringReturn<TYPE_LEN>),
  KeySep1,
  Type(TemporaryStringState<64>, TemporaryStringReturn<64>),
  KeySep2(MessageType),
  SendMessageState(SendMessageState),
  DelegateMessageState(DelegateMessageState),
  UndelegateMessageState(UndelegateMessageState),
  End,
}

fn init_str<const N: usize>() -> <JsonStringAccumulate<N> as JsonInterp<JsonString>>::State {
    <JsonStringAccumulate<N> as JsonInterp<JsonString>>::init(&JsonStringAccumulate)
}
fn call_str<'a, const N: usize>(ss: &mut <JsonStringAccumulate<N> as JsonInterp<JsonString>>::State, token: JsonToken<'a>, dest: &mut Option<<JsonStringAccumulate<N> as JsonInterp<JsonString>>::Returning>) -> Result<(), Option<OOB>> {
    <JsonStringAccumulate<N> as JsonInterp<JsonString>>::parse(&JsonStringAccumulate, ss, token, dest)
}

pub enum MessageReturn<
    SendMessageReturn,
    DelegateMessageReturn,
    UndelegateMessageReturn> {
  SendMessageReturn(Option<SendMessageReturn>),
  DelegateMessageReturn(Option<DelegateMessageReturn>),
  UndelegateMessageReturn(Option<UndelegateMessageReturn>)
}

impl JsonInterp<MessageSchema> for DropInterp {
    type State = <DropInterp as JsonInterp<JsonAny>>::State;
    type Returning = <DropInterp as JsonInterp<JsonAny>>::Returning;
    fn init(&self) -> Self::State {
        <DropInterp as JsonInterp<JsonAny>>::init(&DropInterp)
    }
    fn parse<'a>(&self, state: &mut Self::State, token: JsonToken<'a>, destination: &mut Option<Self::Returning>) -> Result<(), Option<OOB>> {
        <DropInterp as JsonInterp<JsonAny>>::parse(&DropInterp, state, token, destination)
    }
}
macro_rules! unsafe_project_sum {
    ($variant:path, $r0:expr) => {
        match * $r0 {
            $variant(ref mut r) => r,
            _ => core::hint::unreachable_unchecked(),
        }
    }
}

impl <SendInterp: JsonInterp<SendValueSchema>,
      DelegateInterp: JsonInterp<DelegateValueSchema>,
      UndelegateInterp: JsonInterp<UndelegateValueSchema>>
  JsonInterp<MessageSchema> for Message<SendInterp, DelegateInterp, UndelegateInterp>
  where
  <SendInterp as JsonInterp<SendValueSchema>>::State: core::fmt::Debug,
  <DelegateInterp as JsonInterp<DelegateValueSchema>>::State: core::fmt::Debug,
  <UndelegateInterp as JsonInterp<UndelegateValueSchema>>::State: core::fmt::Debug {
  type State = MessageState<<SendInterp as JsonInterp<SendValueSchema>>::State,
                            <DelegateInterp as JsonInterp<DelegateValueSchema>>::State,
                            <UndelegateInterp as JsonInterp<UndelegateValueSchema>>::State>;
  type Returning = MessageReturn<<SendInterp as JsonInterp<SendValueSchema>>::Returning,
                                 <DelegateInterp as JsonInterp<DelegateValueSchema>>::Returning,
                                 <UndelegateInterp as JsonInterp<UndelegateValueSchema>>::Returning>;
  fn init(&self) -> Self::State {
    MessageState::Start
  }
  #[inline(never)]
  fn parse<'a>(&self,
               state: &mut Self::State,
               token: JsonToken<'a>,
               destination: &mut Option<Self::Returning>)
               -> Result<(), Option<OOB>> {
    match state {
      MessageState::Start if token == JsonToken::BeginObject => {
        set_from_thunk(state, ||MessageState::TypeLabel(init_str::<TYPE_LEN>(), None));
      }
      MessageState::TypeLabel(ref mut temp_string_state, ref mut temp_string_return) => {
        call_str::<TYPE_LEN>(temp_string_state, token, temp_string_return)?;
        if temp_string_return.as_ref().unwrap().as_slice() == &TYPE {
          set_from_thunk(state, ||MessageState::KeySep1);
        } else {
          return Err(Some(OOB::Reject))
        }
      }
      MessageState::KeySep1 if token == JsonToken::NameSeparator => {
        set_from_thunk(state, ||MessageState::Type(init_str::<64>(), None));
      }
      MessageState::Type(ref mut temp_string_state, ref mut temp_string_return) => {
        call_str::<64>(temp_string_state, token, temp_string_return)?;
        match temp_string_return.as_ref().unwrap().as_slice() {
          b"/cosmos.bank.v1beta1.MsgSend" =>  {
            set_from_thunk(state, ||MessageState::KeySep2(MessageType::SendMessage));
          }
          b"/cosmos.staking.v1beta1.MsgDelegate" =>  {
            set_from_thunk(state, ||MessageState::KeySep2(MessageType::DelegateMessage));
          }
          b"/cosmos.staking.v1beta1.MsgUndelegate" =>  {
            set_from_thunk(state, ||MessageState::KeySep2(MessageType::UndelegateMessage));
          }
          _ => return Err(Some(OOB::Reject)),
        }
      }
      MessageState::KeySep2(msg_type) if token == JsonToken::ValueSeparator => {
        match msg_type {
          MessageType::SendMessage => {
            let r0 = destination.insert(MessageReturn::SendMessageReturn(None));
            // OK because we just set above
            let r = unsafe { unsafe_project_sum!(MessageReturn::SendMessageReturn, r0) };
            set_from_thunk(state, ||MessageState::SendMessageState(self.send_message.init()));
            // OK because we just set above
            let s = unsafe { unsafe_project_sum!(MessageState::SendMessageState, state) };
            let _res = self.send_message.parse(s, JsonToken::BeginObject, r);
            // One `{` should be valid but not enough input.
            assert_eq!(_res, Err(None));
          }
          MessageType::DelegateMessage => {
            let r0 = destination.insert(MessageReturn::DelegateMessageReturn(None));
            // OK because we just set above
            let r = unsafe { unsafe_project_sum!(MessageReturn::DelegateMessageReturn, r0) };
            set_from_thunk(state, ||MessageState::DelegateMessageState(self.delegate_message.init()));
            // OK because we just set above
            let s = unsafe { unsafe_project_sum!(MessageState::DelegateMessageState, state) };
            let _res = self.delegate_message.parse(s, JsonToken::BeginObject, r);
            // One `{` should be valid but not enough input.
            assert_eq!(_res, Err(None));
          }
          MessageType::UndelegateMessage => {
            let r0 = destination.insert(MessageReturn::UndelegateMessageReturn(None));
            // OK because we just set above
            let r = unsafe { unsafe_project_sum!(MessageReturn::UndelegateMessageReturn, r0) };
            set_from_thunk(state, ||MessageState::UndelegateMessageState(self.undelegate_message.init()));
            // OK because we just set above
            let s = unsafe { unsafe_project_sum!(MessageState::UndelegateMessageState, state) };
            let _res = self.undelegate_message.parse(s, JsonToken::BeginObject, r);
            // One `{` should be valid but not enough input.
            assert_eq!(_res, Err(None));
          }
        }
      }
      MessageState::SendMessageState(ref mut send_message_state) => {
        let sub_destination = &mut destination.as_mut().ok_or(Some(OOB::Reject))?;
        match sub_destination {
          MessageReturn::SendMessageReturn(send_message_return) => {
            self.send_message.parse(send_message_state, token, send_message_return)?;
            return Ok(())
          }
          _ => {
            return Err(Some(OOB::Reject))
          }
        }
      }
      MessageState::DelegateMessageState(ref mut delegate_message_state) => {
        let sub_destination = &mut destination.as_mut().ok_or(Some(OOB::Reject))?;
        match sub_destination {
          MessageReturn::DelegateMessageReturn(delegate_message_return) => {
            self.delegate_message.parse(delegate_message_state, token, delegate_message_return)?;
            return Ok(())
          }
          _ => {
            return Err(Some(OOB::Reject))
          }
        }
      }
      MessageState::UndelegateMessageState(ref mut undelegate_message_state) => {
        let sub_destination = &mut destination.as_mut().ok_or(Some(OOB::Reject))?;
        match sub_destination {
          MessageReturn::UndelegateMessageReturn(undelegate_message_return) => {
            self.undelegate_message.parse(undelegate_message_state, token, undelegate_message_return)?;
            return Ok(())
          }
          _ => {
            return Err(Some(OOB::Reject))
          }
        }
      }
      MessageState::End if token == JsonToken::EndObject => {
      }
      _ => return Err(Some(OOB::Reject)),
    };
    Err(None)
  }
}

provenance_cmd_definition!{}

#[inline(never)]
pub fn get_get_address_state(
    s: &mut ParsersState,
) -> &mut <GetAddressImplT as InterpParser<Bip32Key>>::State {
    match s {
        ParsersState::GetAddressState(_) => {}
        _ => {
            trace!("Non-same state found; initializing state.");
            *s = ParsersState::GetAddressState(<GetAddressImplT as InterpParser<Bip32Key>>::init(
                &GET_ADDRESS_IMPL,
            ));
        }
    }
    match s {
        ParsersState::GetAddressState(ref mut a) => a,
        _ => {
            panic!("")
        }
    }
}

#[inline(never)]
pub fn get_sign_state(
    s: &mut ParsersState,
) -> &mut <SignImplT as InterpParser<SignParameters>>::State {
    match s {
        ParsersState::SignState(_) => {}
        _ => {
            trace!("Non-same state found; initializing state.");
            *s = ParsersState::SignState(<SignImplT as InterpParser<SignParameters>>::init(
                &SIGN_IMPL,
            ));
        }
    }
    match s {
        ParsersState::SignState(ref mut a) => a,
        _ => {
            panic!("")
        }
    }
}

