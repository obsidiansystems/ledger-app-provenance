// use crate::crypto_helpers::{detecdsa_sign, get_pkh, get_private_key, get_pubkey, Hasher};
use crate::crypto_helpers::{get_pubkey, get_pkh, Hasher, format_signature, compress_public_key};
use crate::interface::*;
use core::pin::Pin;
use arrayvec::ArrayVec;
use core::fmt::Write;
use ledger_parser_combinators::any_of;
use pin_project::pin_project;
use core::future::Future;
use ledger_parser_combinators::async_parser::*;
use ledger_parser_combinators::protobufs::schema::ProtobufWireFormat;
use ledger_parser_combinators::protobufs::async_parser::*;
use ledger_parser_combinators::protobufs::schema::Bytes;
use ledger_parser_combinators::protobufs::schema;
use ledger_parser_combinators::interp::{DropInterp, Buffer,
    DefaultInterp, ObserveBytes, SubInterp
};
use nanos_sdk::ecc::*;
pub use crate::proto::cosmos::tx::v1beta1::*; //{SignDocInterp, TxBodyInterp, TxBody};
pub use crate::proto::cosmos::bank::v1beta1::*; // {MsgSendInterp, MsgSend};
pub use crate::proto::cosmos::base::v1beta1::*; // {CoinUnorderedInterp, Coin};

use ledger_prompts_ui::write_scroller;

use core::task::*;
use core::cell::RefCell;
use alamgu_async_block::*;
use ledger_log::*;

pub static mut ASYNC_TRAMPOLINE : Option<RefCell<FutureTrampoline>> = None;

fn trampoline() -> &'static RefCell<FutureTrampoline> {
    unsafe {
        match ASYNC_TRAMPOLINE {
            Some(ref t) => t,
            None => panic!(),
        }
    }
}

pub struct FutureTrampoline {
    pub fut: Option<Pin<&'static mut (dyn Future<Output = ()> + 'static)>>
    }
pub struct FutureTrampolineRunner;

pub fn run_fut<'a, A: 'static, F: 'a + Future<Output = A>>(ft: &'static RefCell<FutureTrampoline>, fut: F) -> impl Future<Output = A> + 'a {
    async move {
    let mut receiver = None;
    let rcv_ptr: *mut Option<A> = &mut receiver;
    let mut computation = async { unsafe { *rcv_ptr = Some(fut.await); } };
    let dfut : Pin<&mut (dyn Future<Output = ()> + '_)> = unsafe { Pin::new_unchecked(&mut computation) };
    let mut computation_unbound : Pin<&mut (dyn Future<Output = ()> + 'static)> = unsafe { core::mem::transmute(dfut) };

    core::future::poll_fn(|_| {
        match core::mem::take(&mut receiver) {
            Some(r) => Poll::Ready(r),
            None => match ft.try_borrow_mut() {
                Ok(ref mut ft_mut) => {
                    match ft_mut.fut {
                        Some(_) => Poll::Pending,
                        None => {
                            ft_mut.fut = Some(unsafe { core::mem::transmute(computation_unbound.as_mut()) });
                            Poll::Pending
                        }
                    }
                }
                Err(_) => Poll::Pending,
            }
        }
    }).await
    }
}

impl AsyncTrampoline for FutureTrampolineRunner {
    fn handle_command(&mut self) -> AsyncTrampolineResult {

        error!("Running trampolines");
        let mut the_fut = match trampoline().try_borrow_mut() {
            Ok(mut futref) => match &mut *futref {
                FutureTrampoline { fut: ref mut pinned } => {
                    core::mem::take(pinned)
                }
            },
            Err(_) => { error!("Case 2"); panic!("Nope"); }
        };
        match the_fut {
            Some(ref mut pinned) => {
                let waker = unsafe { Waker::from_raw(RawWaker::new(&(), &RAW_WAKER_VTABLE)) };
                let mut ctxd = Context::from_waker(&waker);
                match pinned.as_mut().poll(&mut ctxd) {
                    Poll::Pending => AsyncTrampolineResult::Pending,
                    Poll::Ready(()) => AsyncTrampolineResult::Resolved
                }
            }
            None => { AsyncTrampolineResult::NothingPending }
        }
    }
}
/*
struct TrampolineParse<S>(S);

impl<T, S: HasOutput<T>> HasOutput<T> for TrampolineParse<S> {
    type Output = S::Output;
}

impl<T: 'static, BS: Readable, S: LengthDelimitedParser<T, BS>> LengthDelimitedParser<T, BS> for TrampolineParse<S> where S::Output: 'static + Clone {
    type State<'c> = impl Future<Output = Self::Output>;
    fn parse<'a: 'c, 'b: 'c, 'c>(&'b self, input: &'a mut BS, length: usize) -> Self::State<'c> {
            run_fut(trampoline(), self.0.parse(input, length))
    }
}
*/

#[derive(Copy, Clone)]
pub struct GetAddress; // (pub GetAddressImplT);

impl AsyncAPDU for GetAddress {
    // const MAX_PARAMS : usize = 1;
    type State<'c> = impl Future<Output = ()>;

    fn run<'c>(self, io: HostIO, input: ArrayVec<ByteStream, MAX_PARAMS >) -> Self::State<'c> {
        async move {
            error!("Doing getAddress");

            let path = BIP_PATH_PARSER.parse(&mut input[0].clone()).await;

            let _sig = {
                error!("Handling getAddress trampoline call");
                let prompt_fn = || {
                    let pubkey = get_pubkey(&path).ok()?; // Secp256k1::from_bip32(&path).public_key().ok()?;
                    let pkh = get_pkh(&pubkey).ok()?;
                    write_scroller("Provide Public Key", |w| Ok(write!(w, "{}", pkh)?))?;
                    Some((pubkey, pkh))
                };
                if let Some((pubkey, pkh)) = prompt_fn() {
                    error!("Producing Output");
                    let mut rv = ArrayVec::<u8, 128>::new();
                    rv.push(pubkey.len() as u8);
                    
                    // We statically know the lengths of
                    // these slices and so that these will
                    // succeed.
                    let _ = rv.try_extend_from_slice(&pubkey);
                    let _ = rv.try_extend_from_slice(&pkh.0);
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
        input: ArrayVec<ByteStream, MAX_PARAMS>
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

const fn show_address<BS: Readable>(_msg: &'static str) -> impl LengthDelimitedParser<schema::String, BS> {
    Buffer::<120>
        /*
    TrampolineParse(Action(
        Buffer::<120>, move |pkh| {
                        write_scroller(msg, |w| Ok(write!(w, "Foo")?))
        }
    ))*/
}

/*type SMBS = impl Readable + Clone;
const SHOW_SEND_MESSAGE : impl LengthDelimitedParser<MsgSend, dyn Readable + Clone> + HasOutput<MsgSend> =
*/

const fn show_coin<BS: 'static + Readable + ReadableLength + Clone>() -> impl LengthDelimitedParser<Coin, BS> {
    // Action(
        CoinUnorderedInterp {
            field_denom: Buffer::<20>,
            field_amount: Buffer::<100>
     /*   },
    move |_| {
        // write_scroller("Amount", |w| Ok(write!(w, "Faked")?))
        Some(())
     */
    }
    // )
}

// Transaction parser; this should prompt the user a lot more than this.
type TxnParserType = impl LengthDelimitedParser<Transaction, LengthTrack<ByteStream>>;
const TXN_PARSER : TxnParserType = // impl LengthDelimitedParser<Transaction, ByteStream> /*+ HasOutput<Transaction, Output = ()> */ =
    SignDocInterp {
        field_body_bytes: 
            BytesAsMessage(TxBody,
                TxBodyInterp {
                    field_messages: MessagesInterp {
                        send:
                            MsgSendInterp {
                                field_from_address: show_address("From address"),
                                field_to_address: show_address("To address"),
                                field_amount: show_coin()
                                   /* CoinUnorderedInterp {
                                    field_denom: Buffer::<20>,
                                    field_amount: Buffer::<100>
                                }*/
                            }
                        //                                      show_send_message()
                    },
                    field_memo: DropInterp,
                    field_timeout_height: DropInterp,
                    field_extension_options: DropInterp,
                    field_non_critical_extension_options: DropInterp
                }
            ),
            field_auth_info_bytes: DropInterp,
            field_chain_id: DropInterp,
            field_account_number: DropInterp
    };


const fn hasher_parser() -> impl LengthDelimitedParser<Bytes, ByteStream> + HasOutput<Bytes, Output = (Hasher, Option<()>)> { ObserveBytes(Hasher::new, Hasher::update, DropInterp) }

any_of! {
    MessagesInterp {
        Send: MsgSend = b"/cosmos.bank.v1beta1.MsgSend"
    }
    }

type BipPathParserType = impl AsyncParser<Bip32Key, ByteStream> + HasOutput<Bip32Key, Output=ArrayVec<u32, 10>>;
const BIP_PATH_PARSER : BipPathParserType = // Action(
    SubInterp(DefaultInterp); /*,
    // And ask the user if this is the key the meant to sign with:
    mkfn(|path: &ArrayVec<u32, 10>, destination: &mut _| {

        /*let privkey = get_private_key(path).ok()?;
        let pubkey = get_pubkey(path).ok()?; // Redoing work here; fix.
        let pkh = get_pkh(pubkey).ok()?;

        write_scroller("With PKH", |w| Ok(write!(w, "{}", pkh)?))?;*/

        *destination = Some(path.clone());
        Some(())
    }));*/

// #[rustc_layout(debug)]
// type Q<'c> = <Sign as AsyncAPDU>::State<'c>;

impl AsyncAPDU for Sign {
    // const MAX_PARAMS : usize = 2;

    type State<'c> = impl Future<Output = ()>;

    fn run<'c>(self, io: HostIO, mut input: ArrayVec<ByteStream, MAX_PARAMS>) -> Self::State<'c> {
        async move {
            let length = usize::from_le_bytes(input[0].read().await);
            trace!("Passed length");

            {
            let mut txn = LengthTrack(input[0].clone(), 0);
            TXN_PARSER.parse(&mut txn, length).await;
            trace!("Passed txn");
            }

            let hash;

            {
                let mut txn = input[0].clone();
            hash = hasher_parser().parse(&mut txn, length).await.0.finalize();
            trace!("Hashed txn");
            }

            let path = BIP_PATH_PARSER.parse(&mut input[1].clone()).await;
            /*let path : ArrayVec<u32, 10> = run_fut(trampoline(), async move {
                let mut key = input[1].clone();
                PRIVKEY_PARSER.parse(&mut key).await
            }).await;*/

            if let Some(sig) = run_fut(trampoline(), async {
                let sk = Secp256k1::from_bip32(&path);
                let prompt_fn = || {
                    let pkh = get_pkh(&compress_public_key(sk.public_key().ok()?)).ok()?;
                    write_scroller("With PKH", |w| Ok(write!(w, "{}", pkh)?))
                };
                if prompt_fn().is_none() { reject::<()>().await; }

                format_signature(&sk.deterministic_sign(&hash.0[..]).ok()?)
            } ).await {
                io.result_final(&sig).await;
            } else {
                reject::<()>().await;
            }
        }
    }
}

impl<'d> AsyncAPDUStated<ParsersStateCtr> for Sign {
    #[inline(never)]
    fn init<'a, 'b: 'a>(
        self,
        s: &mut core::pin::Pin<&'a mut ParsersState<'a>>,
        io: HostIO,
        input: ArrayVec<ByteStream, MAX_PARAMS>
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

impl <'a>ParsersState<'a> {
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
