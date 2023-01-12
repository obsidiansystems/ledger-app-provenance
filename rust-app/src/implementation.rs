// use crate::crypto_helpers::{detecdsa_sign, get_pkh, get_private_key, get_pubkey, Hasher};
use crate::crypto_helpers::{
    compress_public_key, format_signature, get_pkh, get_pubkey, Hash, Hasher,
};
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
use ledger_parser_combinators::protobufs::schema;
use ledger_parser_combinators::protobufs::schema::Bytes;
use ledger_parser_combinators::protobufs::schema::ProtobufWireFormat;
use nanos_sdk::ecc::*;
use pin_project::pin_project;

// use ledger_prompts_ui::{final_accept_prompt, ScrollerError};

use alamgu_async_block::prompts::*;
use alamgu_async_block::*;
use core::cell::RefCell;
use core::task::*;
use ledger_log::*;

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

#[pin_project]
pub struct NoinlineFut<F: Future>(#[pin] F);

impl<F: Future> Future for NoinlineFut<F> {
    type Output = F::Output;
    #[inline(never)]
    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> core::task::Poll<Self::Output> {
        self.project().0.poll(cx)
    }
}

const fn size_of_val_c<T>(_: &T) -> usize {
    core::mem::size_of::<T>()
}

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
            Some(ref mut pinned) => match poll_with_trivial_context(pinned.as_mut()) {
                Poll::Pending => AsyncTrampolineResult::Pending,
                Poll::Ready(()) => AsyncTrampolineResult::Resolved,
            },
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

async fn get_address_apdu(io: HostIO) {
    let mut input = io.get_params::<1>().unwrap();
    error!("Doing getAddress");

    let path = BIP_PATH_PARSER.parse(&mut input[0].clone()).await;

    error!("Got path");

    let _sig = {
        error!("Handling getAddress trampoline call");
        let prompt_fn = || {
            let pubkey = get_pubkey(&path).ok()?; // Secp256k1::from_bip32(&path).public_key().ok()?;
            let pkh = get_pkh(&pubkey).ok()?;
            error!("Prompting for {}", pkh);
            /*write_scroller("Provide Public Key", |w| {
                Ok(write!(w, "For Address {}", pkh)?)
            })?;
            final_accept_prompt(&[])?;*/
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
    {$prompts_fn:ident, $n: literal, $msg:literal}
    => {
        FutAction(
            Buffer::<$n>, async move |pkh: ArrayVec<u8, $n>| -> Option<()> {
                $prompts_fn().add_prompt($msg, format_args!("{}", core::str::from_utf8(pkh.as_slice()).ok()?)).await;
                Some(())
            }
        )
    };
    {$prompts_fn: ident, ifnonempty, $n: literal, $msg:literal}
    => {
        FutAction(
            Buffer::<$n>, async move |pkh: ArrayVec<u8, $n>| -> Option<()> {
                if !pkh.is_empty() {
                    $prompts_fn().add_prompt($msg, format_args!("{}", core::str::from_utf8(pkh.as_slice()).ok()?)).await;
                }
                Some(())
            }
        )
    }
}

/*const fn show_address<F: Fn(ArrayVec<u8, 120>)->Option<()>>(msg: &'static str) -> Action<Buffer<120>, F> { // impl LengthDelimitedParser<schema::String, BS> {
    // Buffer::<120>
    Action(
        Buffer::<120>, move |pkh| {
                        write_scroller(msg, |w| Ok(write!(w, "{:?}", pkh)?))
        }
    )
}*/

/*type SMBS = impl Readable + Clone;
const SHOW_SEND_MESSAGE : impl LengthDelimitedParser<MsgSend, dyn Readable + Clone> + HasOutput<MsgSend> =
*/

const fn show_coin<BS: 'static + Readable + ReadableLength + Clone>(
) -> impl LengthDelimitedParser<Coin, BS> {
    FutAction(
        CoinUnorderedInterp {
            field_denom: Buffer::<20>,
            field_amount: Buffer::<100>,
        },
        async move |CoinValue {
                        field_denom,
                        field_amount,
                    }: CoinValue<
            Option<ArrayVec<u8, 20>>,
            Option<ArrayVec<u8, 100>>,
        >| {
            // Consider shifting the decimals for nhash to hash here.
            let x = core::str::from_utf8(field_amount.as_ref()?.as_slice()).ok()?;
            let y = core::str::from_utf8(field_denom.as_ref()?.as_slice()).ok()?;
            get_msgs_prompts()
                .add_prompt("Amount", format_args!("{} {}", x, y))
                .await
                .ok()?;
            Some(())
        },
    )
}

// Transaction parser; this should prompt the user a lot more than this.
type TxnParserType = impl LengthDelimitedParser<Transaction, LengthTrack<ByteStream>>
    + HasOutput<Transaction, Output = bool>;
const TXN_PARSER: TxnParserType = TryParser(SignDocInterp {
    field_body_bytes: BytesAsMessage(
        TxBody,
        TxBodyUnorderedInterp {
            field_messages: DropInterp,
            field_memo: show_string!(get_txn_prompts, ifnonempty, 128, "Memo"), // DropInterp,
            field_timeout_height: DropInterp,
            field_extension_options: DropInterp, // Action(DropInterp, |_| { None::<()> }),
            field_non_critical_extension_options: DropInterp, // Action(DropInterp, |_| { None::<()> }),
        },
    ),
    // We could verify that our signature matters with these, but not part of the defining
    // what will the transaction _do_.
    field_auth_info_bytes: DropInterp,
    field_chain_id: show_string!(get_txn_prompts, 20, "Chain ID"),
    field_account_number: DropInterp,
});

struct Preaction<S, F: Future>(fn() -> F, S);

impl<T, S: HasOutput<T>, F: Future> HasOutput<T> for Preaction<S, F> {
    type Output = S::Output;
}

impl<Schema, S: LengthDelimitedParser<Schema, BS>, F: Future, BS: Readable>
    LengthDelimitedParser<Schema, BS> for Preaction<S, F>
{
    type State<'c> = impl Future<Output = Self::Output> + 'c where S: 'c, BS: 'c, F: 'c;
    fn parse<'a: 'c, 'b: 'c, 'c>(&'b self, input: &'a mut BS, length: usize) -> Self::State<'c> {
        async move {
            self.0().await;
            self.1.parse(input, length).await
        }
    }
}

static mut MESSAGES_PROMPTS: Option<PromptQueue> = None;

fn get_msgs_prompts() -> &'static mut PromptQueue {
    unsafe { MESSAGES_PROMPTS.as_mut().unwrap() }
}

static mut TXN_PROMPTS: Option<PromptQueue> = None;

fn get_txn_prompts() -> &'static mut PromptQueue {
    unsafe { TXN_PROMPTS.as_mut().unwrap() }
}

fn init_prompts(io: HostIO) {
    unsafe {
        MESSAGES_PROMPTS = Some(PromptQueue::new(io));
        TXN_PROMPTS = Some(PromptQueue::new(io));
    }
}

const BLANK_FORMAT: core::fmt::Arguments = format_args!("");

type TxnMessagesParser = impl LengthDelimitedParser<Transaction, LengthTrack<ByteStream>>
    + HasOutput<Transaction, Output = bool>;
const TXN_MESSAGES_PARSER: TxnMessagesParser = TryParser(SignDocUnorderedInterp {
    field_body_bytes: BytesAsMessage(
        TxBody,
        TxBodyUnorderedInterp {
            field_messages: MessagesInterp {
                default: RawAnyInterp {
                    field_type_url: Preaction(
                        async || {
                            get_msgs_prompts()
                                .add_prompt("Unknown", format_args!("Message"))
                                .await;
                            // if no_unsafe { None } else {
                            // write_scroller("Unknown", |w| Ok(write!(w, "Message")?))
                            //}
                        },
                        show_string!(get_msgs_prompts, 120, "Type URL"),
                    ),
                    field_value: DropInterp,
                },
                send: TrampolineParse(Preaction(
                    async || {
                        get_msgs_prompts()
                            .add_prompt("Transfer", BLANK_FORMAT)
                            .await
                    }, // write_scroller("Transfer", |w| Ok(())),
                    MsgSendInterp {
                        field_from_address: show_string!(get_msgs_prompts, 120, "From address"),
                        field_to_address: show_string!(get_msgs_prompts, 120, "To address"),
                        field_amount: show_coin(),
                    },
                )),
                multi_send: TrampolineParse(Preaction(
                    async || {
                        get_msgs_prompts()
                            .add_prompt("Multi-send", BLANK_FORMAT)
                            .await
                    },
                    // || write_scroller("Multi-send", |w| Ok(())),
                    MsgMultiSendInterp {
                        field_inputs: InputInterp {
                            field_address: show_string!(get_msgs_prompts, 120, "From address"),
                            field_coins: show_coin(),
                        },
                        field_outputs: OutputInterp {
                            field_address: show_string!(get_msgs_prompts, 120, "To address"),
                            field_coins: show_coin(),
                        },
                    },
                )),
                delegate: TrampolineParse(Preaction(
                    async || {
                        get_msgs_prompts()
                            .add_prompt("Delegate", BLANK_FORMAT)
                            .await
                    },
                    // || write_scroller("Delegate", |w| Ok(())),
                    MsgDelegateInterp {
                        field_amount: show_coin(),
                        field_delegator_address: show_string!(
                            get_msgs_prompts,
                            120,
                            "Delegator Address"
                        ),
                        field_validator_address: show_string!(
                            get_msgs_prompts,
                            120,
                            "Validator Address"
                        ),
                    },
                )),
                undelegate: TrampolineParse(Preaction(
                    async || {
                        get_msgs_prompts()
                            .add_prompt("Undelegate", BLANK_FORMAT)
                            .await
                    },
                    MsgUndelegateInterp {
                        field_amount: show_coin(),
                        field_delegator_address: show_string!(
                            get_msgs_prompts,
                            120,
                            "Delegator Address"
                        ),
                        field_validator_address: show_string!(
                            get_msgs_prompts,
                            120,
                            "Validator Address"
                        ),
                    },
                )),
                begin_redelegate: TrampolineParse(Preaction(
                    async || {
                        get_msgs_prompts()
                            .add_prompt("Redelegate", BLANK_FORMAT)
                            .await
                    },
                    MsgBeginRedelegateInterp {
                        field_amount: show_coin(),
                        field_delegator_address: show_string!(
                            get_msgs_prompts,
                            120,
                            "Delegator Address"
                        ),
                        field_validator_src_address: show_string!(
                            get_msgs_prompts,
                            120,
                            "From Validator"
                        ),
                        field_validator_dst_address: show_string!(
                            get_msgs_prompts,
                            120,
                            "To Validator"
                        ),
                    },
                )),
                deposit: TrampolineParse(MsgDepositInterp {
                    field_amount: show_coin(),
                    field_depositor: show_string!(get_msgs_prompts, 120, "Depositor Address"),
                    field_proposal_id: FutAction(DefaultInterp, async move |value: u64| {
                        get_msgs_prompts()
                            .add_prompt("Proposal ID", format_args!("{}", value))
                            .await
                            .ok()
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
) -> impl LengthDelimitedParser<Bytes, ByteStream> + HasOutput<Bytes, Output = (Hasher, Option<()>)>
{
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

async fn sign_apdu(io: HostIO) {
    let mut input = io.get_params::<2>().unwrap();
    let length = usize::from_le_bytes(input[0].read().await);
    init_prompts(io);
    trace!("Passed length");
    let hash = Hash([0; 32]);

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

    let mut final_prompts = PromptQueue::new(io);

    if !known_txn {
        final_prompts
            .add_prompt("Blind sign", format_args!("Hash: {}", hash))
            .await;
    } else {
        final_prompts.append(get_msgs_prompts()).await;
        final_prompts.append(get_txn_prompts()).await;
    }

    let path = BIP_PATH_PARSER.parse(&mut input[1].clone()).await;

    {
        let sk = Secp256k1::from_bip32(&path);
        let pkh = (|| get_pkh(&compress_public_key(sk.public_key().ok()?)).ok())().unwrap();
        final_prompts
            .add_prompt("With PKH", format_args!("{}", pkh))
            .await;
    }

    match final_prompts.show().await {
        Ok(true) => {}
        _ => reject().await,
    }

    if let Some(sig) = run_fut(trampoline(), || async {
        let sk = Secp256k1::from_bip32(&path);
        format_signature(&sk.deterministic_sign(&hash.0[..]).ok()?)
    })
    .await
    {
        io.result_final(&sig).await;
    } else {
        reject::<()>().await;
    }
}

pub fn reset_parsers_state(state: &mut Pin<&mut Option<APDUsFuture>>) {
    state.set(None);
}

pub type APDUsFuture = impl Future<Output = ()>;

#[inline(never)]
pub fn handle_apdu_async(io: HostIO, ins: Ins) -> APDUsFuture {
    trace!("Constructing future");
    async move {
        trace!("Dispatching");
        match ins {
            Ins::GetVersion => {}
            Ins::GetPubkey => {
                NoinlineFut(get_address_apdu(io)).await;
            }
            Ins::Sign => {
                trace!("Handling sign");
                NoinlineFut(sign_apdu(io)).await;
            }
            Ins::GetVersionStr => {}
            Ins::Exit => nanos_sdk::exit_app(0),
            _ => {}
        }
    }
}
