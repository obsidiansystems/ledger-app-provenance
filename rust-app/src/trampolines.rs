use core::pin::Pin;
use pin_project::pin_project;
use core::future::Future;
use ledger_parser_combinators::async_parser::*;
use ledger_parser_combinators::protobufs::async_parser::*;
pub use crate::proto::cosmos::tx::v1beta1::*;
pub use crate::proto::cosmos::bank::v1beta1::*;
pub use crate::proto::cosmos::base::v1beta1::*;
pub use crate::proto::cosmos::staking::v1beta1::*;
pub use crate::proto::cosmos::gov::v1beta1::*;


use core::task::*;
use core::cell::RefCell;
use alamgu_async_block::*;
use ledger_log::*;

pub static mut ASYNC_TRAMPOLINE : Option<RefCell<FutureTrampoline>> = None;

pub fn trampoline() -> &'static RefCell<FutureTrampoline> {
    unsafe {
        match ASYNC_TRAMPOLINE {
            Some(ref t) => t,
            None => panic!(),
        }
    }
}


// Trampolined and TrampolinedFuture provide a matchable name fragment for static analysis of
// trampoline-executed futures, to connect with handle_fut_trampoline below.
pub trait TrampolinedFuture {
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()>;
}

#[pin_project]
pub struct Trampolined<F: Future>(#[pin] F);

impl<F: Future<Output = ()>> TrampolinedFuture for Trampolined<F> {
    #[inline(never)]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        self.project().0.poll(cx)
    }
}

pub struct FutureTrampoline {
    pub fut: Option<Pin<&'static mut (dyn TrampolinedFuture + 'static)>>
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

pub fn run_fut<'a, A: 'static, F: 'a + Future<Output = A>, FF: 'a + FnOnce() -> F>(ft: &'static RefCell<FutureTrampoline>, fut: FF) -> impl Future<Output = A> + 'a {
    async move {
    let mut receiver = None;
    let rcv_ptr: *mut Option<A> = &mut receiver;
    let mut computation = Trampolined(async { unsafe { *rcv_ptr = Some(fut().await); } });
    let dfut : Pin<&mut (dyn TrampolinedFuture + '_)> = unsafe { Pin::new_unchecked(&mut computation) };
    let mut computation_unbound : Pin<&mut (dyn TrampolinedFuture + 'static)> = unsafe { core::mem::transmute(dfut) };


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

// handle_fut_trampoline is a bare function without mangling so that we can insert it as a caller
// to all TrampolinedFuture poll functions, to reconstruct the missing dyn trait edge in the call
// graph.
//
#[inline(never)]
#[no_mangle]
fn handle_fut_trampoline() -> AsyncTrampolineResult {
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

impl AsyncTrampoline for FutureTrampolineRunner {
    fn handle_command(&mut self) -> AsyncTrampolineResult {
        handle_fut_trampoline()
    }
}

pub struct TrampolineParse<S>(pub S);

impl<T, S: HasOutput<T>> HasOutput<T> for TrampolineParse<S> {
    type Output = S::Output;
}

impl<T: 'static, BS: Readable + ReadableLength, S: LengthDelimitedParser<T, BS>> LengthDelimitedParser<T, BS> for TrampolineParse<S> where S::Output: 'static + Clone {
    type State<'c> = impl Future<Output = Self::Output> where BS: 'c, S: 'c;
    fn parse<'a: 'c, 'b: 'c, 'c>(&'b self, input: &'a mut BS, length: usize) -> Self::State<'c> {
            run_fut(trampoline(), move || self.0.parse(input, length))
    }
}

