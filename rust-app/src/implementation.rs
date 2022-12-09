// use crate::crypto_helpers::{detecdsa_sign, get_pkh, get_private_key, get_pubkey, Hasher};
use crate::crypto_helpers::{compress_public_key, format_signature, get_pkh, get_pubkey, Hasher};
use crate::interface::*;
pub use crate::proto::cosmos::bank::v1beta1::*;
pub use crate::proto::cosmos::base::v1beta1::*;
pub use crate::proto::cosmos::gov::v1beta1::*;
pub use crate::proto::cosmos::staking::v1beta1::*;
pub use crate::proto::cosmos::tx::v1beta1::*;
use arrayvec::{ArrayString, ArrayVec};
use core::fmt::Write;
use core::future::Future;
use ledger_parser_combinators::any_of;
use ledger_parser_combinators::async_parser::*;
use ledger_parser_combinators::interp::{
    Action, Buffer, DefaultInterp, DropInterp, ObserveBytes, SubInterp,
};
use ledger_parser_combinators::protobufs::async_parser::*;
use ledger_parser_combinators::protobufs::schema::Bytes;
use ledger_parser_combinators::protobufs::schema::ProtobufWireFormat;
use nanos_sdk::ecc::*;

use ledger_prompts_ui::{final_accept_prompt, write_scroller, ScrollerError};

use alamgu_async_block::*;
use ledger_log::*;

use crate::trampolines::*;

struct TryParser<S>(S);

impl<T, S: HasOutput<T>> HasOutput<T> for TryParser<S> {
    type Output = bool; // Option<S::Output>;
}

impl<T: 'static, BS: Readable + ReadableLength, S: LengthDelimitedParser<T, BS>>
    LengthDelimitedParser<T, BS> for TryParser<S>
where
    S::Output: 'static,
{
    type State<'c> = impl Future<Output = Self::Output> where BS: 'c, S: 'c;
    fn parse<'a: 'c, 'b: 'c, 'c>(&'b self, input: &'a mut BS, length: usize) -> Self::State<'c> {
        async move { TryFuture(self.0.parse(input, length)).await.is_some() }
    }
}

pub fn get_address_apdu(io: HostIO) -> impl Future<Output = ()> {
    async move {
        let input = io.get_params::<1>().unwrap();
        error!("Doing getAddress");

        let path = BIP_PATH_PARSER.parse(&mut input[0].clone()).await;

        error!("Got path");

        let _sig = {
            error!("Handling getAddress trampoline call");
            let prompt_fn = || {
                let pubkey = get_pubkey(&path).ok()?;
                let pkh = get_pkh(&pubkey).ok()?;
                error!("Prompting for {}", pkh);
                write_scroller("Provide Public Key", |w| {
                    Ok(write!(w, "For Address {}", pkh)?)
                })?;
                final_accept_prompt(&[])?;
                Some((pubkey, pkh))
            };
            if let Some((pubkey, pkh)) = prompt_fn() {
                error!("Producing Output");
                let mut rv = ArrayVec::<u8, 128>::new();

                // We statically know rv is large enough for all of this stuff and the write! can't
                // fail, so we're skipping the results.

                rv.push((pubkey.len()) as u8);
                let _ = rv.try_extend_from_slice(&pubkey).unwrap();
                let mut temp_fmt = ArrayString::<50>::new();
                write!(temp_fmt, "{}", pkh).unwrap();
                rv.push(temp_fmt.as_bytes().len() as u8);
                rv.try_extend_from_slice(temp_fmt.as_bytes()).unwrap();
                io.result_final(&rv).await;
            } else {
                reject::<()>().await;
            }
        };
    }
}

// We'd like this to be just a const fn, but the resulting closure rather than function pointer seems to crash the app.
macro_rules! show_string {
    {$n: literal, $msg:literal}
    => {
        Action(
            Buffer::<$n>, |pkh: ArrayVec<u8, $n>| {
                write_scroller($msg, |w| Ok(write!(w, "{}", core::str::from_utf8(pkh.as_slice())?)?))
            }
        )
    };
    {ifnonempty, $n: literal, $msg:literal}
    => {
        Action(
            Buffer::<$n>, |pkh: ArrayVec<u8, $n>| {
                if pkh.is_empty() { Some(()) } else {
                    write_scroller($msg, |w| Ok(write!(w, "{}", core::str::from_utf8(pkh.as_slice())?)?))
                }
            }
        )
    }
}

const fn show_coin<BS: 'static + Readable + ReadableLength + Clone>(
) -> impl LengthDelimitedParser<Coin, BS> {
    Action(
        CoinUnorderedInterp {
            field_denom: Buffer::<20>,
            field_amount: Buffer::<100>,
        },
        move |CoinValue {
                  field_denom,
                  field_amount,
              }: CoinValue<Option<ArrayVec<u8, 20>>, Option<ArrayVec<u8, 100>>>| {
            // Consider shifting the decimals for nhash to hash here.
            write_scroller("Amount", |w| {
                let x =
                    core::str::from_utf8(field_amount.as_ref().ok_or(ScrollerError)?.as_slice())?;
                let y =
                    core::str::from_utf8(field_denom.as_ref().ok_or(ScrollerError)?.as_slice())?;
                write!(w, "{} {}", x, y).map_err(|_| ScrollerError) // TODO don't map_err
            })
        },
    )
}

// Transaction parser; this should prompt the user a lot more than this.
type TxnParserType = impl LengthDelimitedParser<Transaction, LengthTrack<ByteStream>>
    + HasOutput<Transaction, Output = bool>;
const TXN_PARSER: TxnParserType = TryParser(SignDocInterp {
    field_body_bytes: BytesAsMessage(
        TxBody,
        TxBodyInterp {
            field_messages: DropInterp,
            field_memo: show_string!(ifnonempty, 128, "Memo"), // DropInterp,
            field_timeout_height: DropInterp,
            field_extension_options: DropInterp, // Action(DropInterp, |_| { None::<()> }),
            field_non_critical_extension_options: DropInterp, // Action(DropInterp, |_| { None::<()> }),
        },
    ),
    // We could verify that our signature matters with these, but not part of the defining
    // what will the transaction _do_.
    field_auth_info_bytes: DropInterp,
    field_chain_id: show_string!(20, "Chain ID"),
    field_account_number: DropInterp,
});

struct Preaction<S>(fn() -> Option<()>, S);

impl<T, S: HasOutput<T>> HasOutput<T> for Preaction<S> {
    type Output = S::Output;
}

impl<Schema, S: LengthDelimitedParser<Schema, BS>, BS: Readable> LengthDelimitedParser<Schema, BS>
    for Preaction<S>
{
    type State<'c> = impl Future<Output = Self::Output> where S: 'c, BS: 'c;
    fn parse<'a: 'c, 'b: 'c, 'c>(&'b self, input: &'a mut BS, length: usize) -> Self::State<'c> {
        async move {
            if self.0().is_none() {
                reject().await
            } else {
                self.1.parse(input, length).await
            }
        }
    }
}

type TxnMessagesParser = impl LengthDelimitedParser<Transaction, LengthTrack<ByteStream>>
    + HasOutput<Transaction, Output = bool>;
const TXN_MESSAGES_PARSER: TxnMessagesParser = TryParser(SignDocUnorderedInterp {
    field_body_bytes: BytesAsMessage(
        TxBody,
        TxBodyUnorderedInterp {
            field_messages: MessagesInterp {
                default: RawAnyInterp {
                    field_type_url: Preaction(
                        || {
                            // if no_unsafe { None } else {
                            write_scroller("Unknown", |w| Ok(write!(w, "Message")?))
                            //}
                        },
                        show_string!(120, "Type URL"),
                    ),
                    field_value: DropInterp,
                },
                send: TrampolineParse(Preaction(
                    || write_scroller("Transfer", |_| Ok(())),
                    MsgSendInterp {
                        field_from_address: show_string!(120, "From address"),
                        field_to_address: show_string!(120, "To address"),
                        field_amount: show_coin(),
                    },
                )),
                multi_send: TrampolineParse(Preaction(
                    || write_scroller("Multi-send", |_| Ok(())),
                    MsgMultiSendInterp {
                        field_inputs: InputInterp {
                            field_address: show_string!(120, "From address"),
                            field_coins: show_coin(),
                        },
                        field_outputs: OutputInterp {
                            field_address: show_string!(120, "To address"),
                            field_coins: show_coin(),
                        },
                    },
                )),
                delegate: TrampolineParse(Preaction(
                    || write_scroller("Delegate", |_| Ok(())),
                    MsgDelegateInterp {
                        field_amount: show_coin(),
                        field_delegator_address: show_string!(120, "Delegator Address"),
                        field_validator_address: show_string!(120, "Validator Address"),
                    },
                )),
                undelegate: TrampolineParse(Preaction(
                    || write_scroller("Undelegate", |_| Ok(())),
                    MsgUndelegateInterp {
                        field_amount: show_coin(),
                        field_delegator_address: show_string!(120, "Delegator Address"),
                        field_validator_address: show_string!(120, "Validator Address"),
                    },
                )),
                begin_redelegate: TrampolineParse(Preaction(
                    || write_scroller("Redelegate", |_| Ok(())),
                    MsgBeginRedelegateInterp {
                        field_amount: show_coin(),
                        field_delegator_address: show_string!(120, "Delegator Address"),
                        field_validator_src_address: show_string!(120, "From Validator"),
                        field_validator_dst_address: show_string!(120, "To Validator"),
                    },
                )),
                deposit: TrampolineParse(MsgDepositInterp {
                    field_amount: show_coin(),
                    field_depositor: show_string!(120, "Depositor Address"),
                    field_proposal_id: Action(DefaultInterp, |value: u64| {
                        write_scroller("Proposal ID", |w| Ok(write!(w, "{}", value)?))
                    }),
                }),
            },
            field_memo: DropInterp,
            field_timeout_height: DropInterp,
            field_extension_options: DropInterp,
            field_non_critical_extension_options: DropInterp,
        },
    ),
    field_auth_info_bytes: DropInterp,
    field_chain_id: DropInterp,
    field_account_number: DropInterp,
});

type HasherParser =
    impl LengthDelimitedParser<Bytes, ByteStream> + HasOutput<Bytes, Output = (Hasher, Option<()>)>;
const fn hasher_parser() -> HasherParser {
    ObserveBytes(Hasher::new, Hasher::update, DropInterp)
}

any_of! {
MessagesInterp {
    Send: MsgSend = b"/cosmos.bank.v1beta1.MsgSend",
    MultiSend: MsgMultiSend = b"/cosmos.bank.v1beta1.MsgMultiSend",
    Delegate: MsgDelegate = b"/cosmos.staking.v1beta1.MsgDelegate",
    Undelegate: MsgUndelegate = b"/cosmos.staking.v1beta1.MsgUndelegate",
    BeginRedelegate: MsgBeginRedelegate = b"/cosmos.staking.v1beta1.MsgBeginRedelegate",
    Deposit: MsgDeposit = b"/cosmos.gov.v1beta1.MsgDeposit"
}
}

type BipPathParserType =
    impl AsyncParser<Bip32Key, ByteStream> + HasOutput<Bip32Key, Output = ArrayVec<u32, 10>>;
const BIP_PATH_PARSER: BipPathParserType =
    Action(SubInterp(DefaultInterp), |path: ArrayVec<u32, 10>| {
        if path.len() < 2 || path[0] != 0x8000002c || path[1] != 0x800001f9 {
            None
        } else {
            Some(path)
        }
    });

pub fn sign_apdu(io: HostIO) -> impl Future<Output = ()> {
    async move {
        let mut input = io.get_params::<2>().unwrap();
        let length = usize::from_le_bytes(input[0].read().await);
        trace!("Passed length");

        let mut known_txn = {
            let mut txn = LengthTrack(input[0].clone(), 0);
            TrampolineParse(TXN_MESSAGES_PARSER)
                .parse(&mut txn, length)
                .await
        };
        trace!("Passed txn messages");

        if known_txn {
            let mut txn = LengthTrack(input[0].clone(), 0);
            known_txn = TrampolineParse(TXN_PARSER).parse(&mut txn, length).await;
            trace!("Passed txn");
        }

        let hash;

        {
            let mut txn = input[0].clone();
            hash = hasher_parser().parse(&mut txn, length).await.0.finalize();
            trace!("Hashed txn");
        }

        if !known_txn {
            if write_scroller("Blind sign hash", |w| Ok(write!(w, "{}", hash)?)).is_none() {
                reject::<()>().await;
            };
        }

        let path = BIP_PATH_PARSER.parse(&mut input[1].clone()).await;

        if let Some(sig) = run_fut(trampoline(), || async {
            let sk = Secp256k1::from_bip32(&path);
            let prompt_fn = || {
                let pkh = get_pkh(&compress_public_key(sk.public_key().ok()?)).ok()?;
                write_scroller("With PKH", |w| Ok(write!(w, "{}", pkh)?))?;
                final_accept_prompt(&[])
            };
            if prompt_fn().is_none() {
                reject::<()>().await;
            }
            format_signature(&sk.deterministic_sign(&hash.0[..]).ok()?)
        })
        .await
        {
            io.result_final(&sig).await;
        } else {
            reject::<()>().await;
        }
    }
}

pub type APDUsFuture = impl Future<Output = ()>;

#[inline(never)]
pub fn handle_apdu_async(io: HostIO, ins: Ins) -> APDUsFuture {
    async move {
        match ins {
            Ins::GetVersion => {}
            Ins::GetPubkey => run_fut(trampoline(), move || get_address_apdu(io)).await,
            Ins::Sign => {
                trace!("Handling sign");
                run_fut(trampoline(), move || sign_apdu(io)).await
            }
            Ins::GetVersionStr => {}
            Ins::Exit => nanos_sdk::exit_app(0),
        }
    }
}
