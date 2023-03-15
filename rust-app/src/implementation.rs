// use crate::crypto_helpers::{detecdsa_sign, get_pkh, get_private_key, get_pubkey, Hasher};
use crate::crypto_helpers::{compress_public_key, format_signature, get_pkh, get_pubkey};
use crate::interface::*;
pub use crate::proto::cosmos::bank::v1beta1::*;
pub use crate::proto::cosmos::base::v1beta1::*;
pub use crate::proto::cosmos::gov::v1beta1::*;
pub use crate::proto::cosmos::staking::v1beta1::*;
pub use crate::proto::cosmos::tx::v1beta1::*;
use crate::utils::*;
use arrayvec::{ArrayString, ArrayVec};
use core::fmt::Write;
use core::future::Future;
use core::pin::Pin;
use ledger_parser_combinators::any_of;
use ledger_parser_combinators::async_parser::*;
use ledger_parser_combinators::interp::{
    Action, Buffer, DefaultInterp, DropInterp, ObserveBytes, SubInterp,
};
use ledger_parser_combinators::protobufs::async_parser::*;
use ledger_parser_combinators::protobufs::schema::Bytes;
use ledger_parser_combinators::protobufs::schema::ProtobufWireFormat;
use nanos_sdk::ecc::*;
use pin_project::pin_project;

use alamgu_async_block::*;
use core::cell::RefCell;
use core::task::*;
use ledger_crypto_helpers::hasher::{Base64Hash, Hasher, SHA256};
use ledger_log::*;
use ledger_prompts_ui::{final_accept_prompt, ScrollerError};

pub static mut ASYNC_TRAMPOLINE: Option<RefCell<FutureTrampoline>> = None;

fn trampoline() -> &'static RefCell<FutureTrampoline> {
    unsafe {
        match ASYNC_TRAMPOLINE {
            Some(ref t) => t,
            None => panic!(),
        }
    }
}

pub struct FutureTrampoline {
    pub fut: Option<Pin<&'static mut (dyn Future<Output = ()> + 'static)>>,
}
pub struct FutureTrampolineRunner;

pub fn run_fut<'a, A: 'static, F: 'a + Future<Output = A>, FF: 'a + FnOnce() -> F>(
    ft: &'static RefCell<FutureTrampoline>,
    fut: FF,
) -> impl Future<Output = A> + 'a {
    async move {
        let mut receiver = None;
        let rcv_ptr: *mut Option<A> = &mut receiver;
        let mut computation = async {
            unsafe {
                *rcv_ptr = Some(fut().await);
            }
        };
        let dfut: Pin<&mut (dyn Future<Output = ()> + '_)> =
            unsafe { Pin::new_unchecked(&mut computation) };
        let mut computation_unbound: Pin<&mut (dyn Future<Output = ()> + 'static)> =
            unsafe { core::mem::transmute(dfut) };

        error!("Waiting for future in run_fut");
        core::future::poll_fn(|_| {
            error!("run_fut poll_fn");
            match core::mem::take(&mut receiver) {
                Some(r) => {
                    error!("run_fut completing");
                    Poll::Ready(r)
                }
                None => match ft.try_borrow_mut() {
                    Ok(ref mut ft_mut) => match ft_mut.fut {
                        Some(_) => Poll::Pending,
                        None => {
                            ft_mut.fut =
                                Some(unsafe { core::mem::transmute(computation_unbound.as_mut()) });
                            Poll::Pending
                        }
                    },
                    Err(_) => Poll::Pending,
                },
            }
        })
        .await
    }
}

impl AsyncTrampoline for FutureTrampolineRunner {
    fn handle_command(&mut self) -> AsyncTrampolineResult {
        error!("Running trampolines");
        let mut the_fut = match trampoline().try_borrow_mut() {
            Ok(mut futref) => match &mut *futref {
                FutureTrampoline {
                    fut: ref mut pinned,
                } => core::mem::take(pinned),
            },
            Err(_) => {
                error!("Case 2");
                panic!("Nope");
            }
        };
        error!("Something is pending");
        match the_fut {
            Some(ref mut pinned) => {
                let waker = unsafe { Waker::from_raw(RawWaker::new(&(), &RAW_WAKER_VTABLE)) };
                let mut ctxd = Context::from_waker(&waker);
                match pinned.as_mut().poll(&mut ctxd) {
                    Poll::Pending => AsyncTrampolineResult::Pending,
                    Poll::Ready(()) => AsyncTrampolineResult::Resolved,
                }
            }
            None => AsyncTrampolineResult::NothingPending,
        }
    }
}

struct TrampolineParse<S>(S);

impl<T, S: HasOutput<T>> HasOutput<T> for TrampolineParse<S> {
    type Output = S::Output;
}

impl<T: 'static, BS: Readable + ReadableLength, S: LengthDelimitedParser<T, BS>>
    LengthDelimitedParser<T, BS> for TrampolineParse<S>
where
    S::Output: 'static + Clone,
{
    type State<'c> = impl Future<Output = Self::Output> + 'c where BS: 'c, S: 'c;
    fn parse<'a: 'c, 'b: 'c, 'c>(&'b self, input: &'a mut BS, length: usize) -> Self::State<'c> {
        run_fut(trampoline(), move || self.0.parse(input, length))
    }
}

struct TryParser<S>(S);

impl<T, S: HasOutput<T>> HasOutput<T> for TryParser<S> {
    type Output = bool; // Option<S::Output>;
}

impl<T: 'static, BS: Readable + ReadableLength, S: LengthDelimitedParser<T, BS>>
    LengthDelimitedParser<T, BS> for TryParser<S>
where
    S::Output: 'static,
{
    type State<'c> = impl Future<Output = Self::Output> + 'c where BS: 'c, S: 'c;
    fn parse<'a: 'c, 'b: 'c, 'c>(&'b self, input: &'a mut BS, length: usize) -> Self::State<'c> {
        async move { TryFuture(self.0.parse(input, length)).await.is_some() }
    }
}

#[derive(Copy, Clone)]
pub struct GetAddress; // (pub GetAddressImplT);

impl AsyncAPDU for GetAddress {
    // const MAX_PARAMS : usize = 1;
    type State<'c> = impl Future<Output = ()>;

    fn run<'c>(self, io: HostIO, input: ArrayVec<ByteStream, MAX_PARAMS>) -> Self::State<'c> {
        async move {
            error!("Doing getAddress");

            let path = BIP_PATH_PARSER.parse(&mut input[0].clone()).await;

            error!("Got path");

            let _sig = {
                error!("Handling getAddress trampoline call");
                let prompt_fn = || {
                    let pubkey = get_pubkey(&path).ok()?; // Secp256k1::derive_from_path(&path).public_key().ok()?;
                    let pkh = get_pkh(&pubkey).ok()?;
                    Some((pubkey, pkh))
                };
                if let Some((pubkey, pkh)) = prompt_fn() {
                    error!("Producing Output");
                    let mut rv = ArrayVec::<u8, 128>::new();
                    rv.push(pubkey.len() as u8);

                    rv.try_extend_from_slice(&pubkey);
                    let mut temp_fmt = ArrayString::<50>::new();
                    write!(temp_fmt, "{}", pkh);

                    // We statically know the lengths of
                    // these slices and so that these will
                    // succeed.
                    let _ = rv.try_push(temp_fmt.as_bytes().len() as u8);
                    let _ = rv.try_extend_from_slice(temp_fmt.as_bytes());

                    io.result_final(&rv).await;
                } else {
                    reject::<()>().await;
                }
            };
        }
    }
}

impl<'d> AsyncAPDUStated<ParsersStateCtr> for GetAddress {
    #[inline(never)]
    fn init<'a, 'b: 'a>(
        self,
        s: &mut core::pin::Pin<&'a mut ParsersState<'a>>,
        io: HostIO,
        input: ArrayVec<ByteStream, MAX_PARAMS>,
    ) -> () {
        s.set(ParsersState::GetAddressState(self.run(io, input)));
    }

    /*
    #[inline(never)]
    fn get<'a, 'b>(self, s: &'b mut core::pin::Pin<&'a mut ParsersState<'a>>) -> Option<&'b mut core::pin::Pin<&'a mut Self::State<'a>>> {
        match s.as_mut().project() {
            ParsersStateProjection::GetAddressState(ref mut s) => Some(s),
            _ => panic!("Oops"),
        }
    }*/

    #[inline(never)]
    fn poll<'a, 'b>(self, s: &mut core::pin::Pin<&'a mut ParsersState>) -> core::task::Poll<()> {
        let waker = unsafe { Waker::from_raw(RawWaker::new(&(), &RAW_WAKER_VTABLE)) };
        let mut ctxd = Context::from_waker(&waker);
        match s.as_mut().project() {
            ParsersStateProjection::GetAddressState(ref mut s) => s.as_mut().poll(&mut ctxd),
            _ => panic!("Ooops"),
        }
    }
}

#[derive(Copy, Clone)]
pub struct Sign;

/*
const fn show_send_message<BS: 'static + Readable + Clone>() -> impl LengthDelimitedParser<MsgSend, BS> + HasOutput<MsgSend> {
    MsgSendInterp {
        field_from_address: DropInterp, //show_address("From address"),
        field_to_address: DropInterp, //show_address("To address"),
        field_amount: CoinInterp {
            field_denom: DropInterp, // Buffer::<20>,
            field_amount: DropInterp, // Buffer::<100>
        }
    }
}
*/

// We'd like this to be just a const fn, but the resulting closure rather than function pointer seems to crash the app.
macro_rules! show_string {
    {$n: literal, $msg:literal}
    => {
        Action(
            Buffer::<$n>, |pkh: ArrayVec<u8, $n>| {
                scroller($msg, |w| Ok(write!(w, "{}", core::str::from_utf8(pkh.as_slice())?)?))
            }
        )
    };
    {ifnonempty, $n: literal, $msg:literal}
    => {
        Action(
            Buffer::<$n>, |pkh: ArrayVec<u8, $n>| {
                if pkh.is_empty() { Some(()) } else {
                    scroller($msg, |w| Ok(write!(w, "{}", core::str::from_utf8(pkh.as_slice())?)?))
                }
            }
        )
    }
}

/*const fn show_address<F: Fn(ArrayVec<u8, 120>)->Option<()>>(msg: &'static str) -> Action<Buffer<120>, F> { // impl LengthDelimitedParser<schema::String, BS> {
    // Buffer::<120>
    Action(
        Buffer::<120>, move |pkh| {
                        scroller(msg, |w| Ok(write!(w, "{:?}", pkh)?))
        }
    )
}*/

/*type SMBS = impl Readable + Clone;
const SHOW_SEND_MESSAGE : impl LengthDelimitedParser<MsgSend, dyn Readable + Clone> + HasOutput<MsgSend> =
*/

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
            scroller("Amount", |w| {
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
    type State<'c> = impl Future<Output = Self::Output> + 'c where S: 'c, BS: 'c;
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
                            scroller("Unknown", |w| Ok(write!(w, "Message")?))
                            //}
                        },
                        show_string!(120, "Type URL"),
                    ),
                    field_value: DropInterp,
                },
                send: TrampolineParse(Action(
                    MsgSendUnorderedInterp {
                        field_from_address: Buffer::<120>,
                        field_to_address: Buffer::<120>,
                        field_amount: CoinUnorderedInterp {
                            field_denom: Buffer::<20>,
                            field_amount: Buffer::<100>,
                        },
                    },
                    |o: MsgSendValue<
                        Option<ArrayVec<u8, 120>>,
                        Option<ArrayVec<u8, 120>>,
                        Option<CoinValue<Option<ArrayVec<u8, 20>>, Option<ArrayVec<u8, 100>>>>,
                    >|
                     -> Option<()> {
                        scroller("Transfer", |w| Ok(write!(w, "HASH")?));
                        scroller_paginated("From", |w| {
                            let x = core::str::from_utf8(
                                o.field_from_address
                                    .as_ref()
                                    .ok_or(ScrollerError)?
                                    .as_slice(),
                            )?;
                            write!(w, "{x}").map_err(|_| ScrollerError) // TODO don't map_err
                        });
                        scroller_paginated("To", |w| {
                            let x = core::str::from_utf8(
                                o.field_to_address.as_ref().ok_or(ScrollerError)?.as_slice(),
                            )?;
                            write!(w, "{x}").map_err(|_| ScrollerError) // TODO don't map_err
                        });
                        scroller("Amount", |w| {
                            let x = core::str::from_utf8(
                                o.field_amount
                                    .as_ref()
                                    .ok_or(ScrollerError)?
                                    .field_amount
                                    .as_ref()
                                    .ok_or(ScrollerError)?
                                    .as_slice(),
                            )?;
                            let y = core::str::from_utf8(
                                o.field_amount
                                    .as_ref()
                                    .ok_or(ScrollerError)?
                                    .field_denom
                                    .as_ref()
                                    .ok_or(ScrollerError)?
                                    .as_slice(),
                            )?;
                            write!(w, "{} {}", x, y).map_err(|_| ScrollerError) // TODO don't map_err
                        })
                    },
                )),
                multi_send: TrampolineParse(Preaction(
                    || scroller("Multi-send", |_| Ok(())),
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
                    || scroller("Delegate", |_| Ok(())),
                    MsgDelegateInterp {
                        field_amount: show_coin(),
                        field_delegator_address: show_string!(120, "Delegator Address"),
                        field_validator_address: show_string!(120, "Validator Address"),
                    },
                )),
                undelegate: TrampolineParse(Preaction(
                    || scroller("Undelegate", |_| Ok(())),
                    MsgUndelegateInterp {
                        field_amount: show_coin(),
                        field_delegator_address: show_string!(120, "Delegator Address"),
                        field_validator_address: show_string!(120, "Validator Address"),
                    },
                )),
                begin_redelegate: TrampolineParse(Preaction(
                    || scroller("Redelegate", |_| Ok(())),
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
                        scroller("Proposal ID", |w| Ok(write!(w, "{}", value)?))
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

const fn hasher_parser(
) -> impl LengthDelimitedParser<Bytes, ByteStream> + HasOutput<Bytes, Output = (SHA256, Option<()>)>
{
    ObserveBytes(SHA256::new, SHA256::update, DropInterp)
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

// #[rustc_layout(debug)]
// type Q<'c> = <Sign as AsyncAPDU>::State<'c>;
//

impl AsyncAPDU for Sign {
    // const MAX_PARAMS : usize = 2;

    type State<'c> = impl Future<Output = ()>;

    fn run<'c>(self, io: HostIO, mut input: ArrayVec<ByteStream, MAX_PARAMS>) -> Self::State<'c> {
        async move {
            let length = usize::from_le_bytes(input[0].read().await);
            trace!("Passed length");

            let mut known_txn = NoinlineFut((|bs: ByteStream| async move {
                {
                    let mut txn = LengthTrack(bs, 0);
                    TXN_MESSAGES_PARSER.parse(&mut txn, length).await
                }
            })(input[0].clone()))
            .await;
            trace!("Passed txn messages");

            if known_txn {
                let mut txn = LengthTrack(input[0].clone(), 0);
                known_txn = TrampolineParse(TXN_PARSER).parse(&mut txn, length).await;
                trace!("Passed txn");
            }

            NoinlineFut((|input: ArrayVec<ByteStream, 2>| async move {
                let hash: Base64Hash<32>;

                {
                    let mut txn = input[0].clone();
                    hash = hasher_parser().parse(&mut txn, length).await.0.finalize();
                    trace!("Hashed txn");
                }

                if !known_txn {
                    if scroller("Blind sign hash", |w| Ok(write!(w, "{}", hash)?)).is_none() {
                        reject::<()>().await;
                    };
                }

                let path = BIP_PATH_PARSER.parse(&mut input[1].clone()).await;

                if let Some(sig) = run_fut(trampoline(), || async {
                    let sk = Secp256k1::derive_from_path(&path);
                    let prompt_fn = || {
                        let pkh = get_pkh(&compress_public_key(sk.public_key().ok()?)).ok()?;
                        scroller("With PKH", |w| Ok(write!(w, "{}", pkh)?))?;
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
            })(input))
            .await
        }
    }
}

impl<'d> AsyncAPDUStated<ParsersStateCtr> for Sign {
    #[inline(never)]
    fn init<'a, 'b: 'a>(
        self,
        s: &mut core::pin::Pin<&'a mut ParsersState<'a>>,
        io: HostIO,
        input: ArrayVec<ByteStream, MAX_PARAMS>,
    ) -> () {
        s.set(ParsersState::SignState(self.run(io, input)));
    }

    #[inline(never)]
    fn poll<'a>(self, s: &mut core::pin::Pin<&'a mut ParsersState>) -> core::task::Poll<()> {
        let waker = unsafe { Waker::from_raw(RawWaker::new(&(), &RAW_WAKER_VTABLE)) };
        let mut ctxd = Context::from_waker(&waker);
        match s.as_mut().project() {
            ParsersStateProjection::SignState(ref mut s) => s.as_mut().poll(&mut ctxd),
            _ => panic!("Ooops"),
        }
    }
}

// The global parser state enum; any parser above that'll be used as the implementation for an APDU
// must have a field here.

// type GetAddressStateType = impl Future;
// type SignStateType = impl Future<Output = ()>;

#[pin_project(project = ParsersStateProjection)]
pub enum ParsersState<'a> {
    NoState,
    GetAddressState(#[pin] <GetAddress as AsyncAPDU>::State<'a>), // <GetAddressImplT<'a> as AsyncParser<Bip32Key, ByteStream<'a>>>::State<'a>),
    SignState(#[pin] <Sign as AsyncAPDU>::State<'a>),
    // SignState(#[pin] <SignImplT<'a> as AsyncParser<SignParameters, ByteStream<'a>>>::State<'a>),
}

impl<'a> ParsersState<'a> {
    pub fn is_no_state(&self) -> bool {
        match self {
            ParsersState::NoState => true,
            _ => false,
        }
    }
}

impl<'a> Default for ParsersState<'a> {
    fn default() -> Self {
        ParsersState::NoState
    }
}

pub fn reset_parsers_state(state: &mut Pin<&mut ParsersState>) {
    state.set(ParsersState::default());
}

// we need to pass a type constructor for ParsersState to various places, so that we can give it
// the right lifetime; this is a bit convoluted, but works.

pub struct ParsersStateCtr;
impl StateHolderCtr for ParsersStateCtr {
    type StateCtr<'a> = ParsersState<'a>;
}
