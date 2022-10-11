use core::pin::Pin;
use core::cell::RefCell;

use alamgu_async_block::*;
use provenance::implementation::*;

use ledger_prompts_ui::RootMenu;

use nanos_sdk::io;
nanos_sdk::set_panic!(nanos_sdk::exiting_panic);
use core::mem::MaybeUninit;

use provenance::*;
use provenance::trampolines::*;

static mut COMM_CELL : MaybeUninit<RefCell<io::Comm>> = MaybeUninit::uninit();
static mut HOST_IO_STATE : MaybeUninit<RefCell<HostIOState>> = MaybeUninit::uninit();
static mut STATES_BACKING : MaybeUninit<Option<APDUsFuture>> = MaybeUninit::uninit();

#[inline(never)]
unsafe fn initialize() {
    STATES_BACKING.write(None);
    COMM_CELL.write(RefCell::new(io::Comm::new()));
    let comm = COMM_CELL.assume_init_ref();
    HOST_IO_STATE.write(RefCell::new(HostIOState {
        comm: comm,
        requested_block: None,
        sent_command: None,
    }));
    ASYNC_TRAMPOLINE = Some(RefCell::new(FutureTrampoline { fut: None }));
}

#[inline(never)]
fn noinline<A>(f: impl FnOnce() -> A) -> A {
    f()
}

#[cfg(not(test))]
#[no_mangle]
extern "C" fn sample_main() {
    unsafe { initialize(); }
    let comm = unsafe { COMM_CELL.assume_init_ref() };
    let host_io = HostIO(unsafe { HOST_IO_STATE.assume_init_ref() });
    let mut states = unsafe { Pin::new_unchecked( STATES_BACKING.assume_init_mut() ) };

    let mut idle_menu = RootMenu::new([ concat!("Provenance ", env!("CARGO_PKG_VERSION")), "Exit" ]);
    let mut busy_menu = RootMenu::new([ "Working...", "Cancel" ]);

    info!("Provenance App {}", env!("CARGO_PKG_VERSION"));
    info!("State sizes\ncomm: {}\nstates: {}\nhostio: {}"
          , core::mem::size_of::<io::Comm>()
          , core::mem::size_of::<APDUsFuture>()
          , core::mem::size_of::<HostIOState>());

    let // Draw some 'welcome' screen
        menu = |states : &Option<_>, idle : & mut RootMenu<2>, busy : & mut RootMenu<2>| {
            match states {
                None => idle.show(),
                _ => busy.show(),
            }
        };

    noinline(|| menu(&states, & mut idle_menu, & mut busy_menu));
    loop {
        // Wait for either a specific button push to exit the app
        // or an APDU command
        let evt = comm.borrow_mut().next_event();
        match evt {
            io::Event::Command(ins) => {
                trace!("Command received");
                match poll_apdu_handlers(&mut states, ins, host_io, &mut FutureTrampolineRunner, handle_apdu_async) {
                    // handle_apdu(host_io, ins, &mut states) {
                    Ok(()) => {
                        trace!("APDU accepted; sending response");
                        comm.borrow_mut().reply_ok();
                        trace!("Replied");
                    }
                    Err(sw) => comm.borrow_mut().reply(sw),
                };
                noinline(|| menu(&states, & mut idle_menu, & mut busy_menu));
                trace!("Command done");
            }
            io::Event::Button(btn) => {
                trace!("Button received");
                match states.is_none() {
                    true => {match noinline(|| idle_menu.update(btn)) {
                        Some(1) => { info!("Exiting app at user direction via root menu"); nanos_sdk::exit_app(0) },
                        _ => (),
                    } }
                    false => { match noinline(|| busy_menu.update(btn)) {
                        Some(1) => { info!("Resetting at user direction via busy menu"); noinline(|| states.set(None)) }
                        _ => (),
                    } }
                };
                menu(&states, & mut idle_menu, & mut busy_menu);
                trace!("Button done");
            }
            io::Event::Ticker => {
                //trace!("Ignoring ticker event");
            },
        }
    }
}


