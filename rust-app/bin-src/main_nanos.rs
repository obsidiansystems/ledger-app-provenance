use core::cell::RefCell;
use core::pin::Pin;
use pin_cell::*;

use alamgu_async_block::*;
use provenance::implementation::*;
use provenance::interface::*;

use ledger_prompts_ui::RootMenu;

use nanos_sdk::io;
nanos_sdk::set_panic!(nanos_sdk::exiting_panic);

use provenance::*;

static mut COMM_CELL: Option<RefCell<io::Comm>> = None;

static mut HOST_IO_STATE: Option<RefCell<HostIOState>> = None;

#[inline(never)]
unsafe fn initialize() {
    COMM_CELL = Some(RefCell::new(io::Comm::new()));
    let comm = COMM_CELL.as_ref().unwrap();
    HOST_IO_STATE = Some(RefCell::new(HostIOState {
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
    unsafe {
        initialize();
    }
    let comm = unsafe { COMM_CELL.as_ref().unwrap() };
    let host_io = HostIO(unsafe { HOST_IO_STATE.as_ref().unwrap() });

    let mut states_backing: PinCell<Option<APDUsFuture>> = PinCell::new(None);
    let states: Pin<&PinCell<Option<APDUsFuture>>> =
        Pin::static_ref(unsafe { core::mem::transmute(&states_backing) });

    let mut idle_menu = RootMenu::new([concat!("Provenance ", env!("CARGO_PKG_VERSION")), "Exit"]);
    let mut busy_menu = RootMenu::new(["Working...", "Cancel"]);

    info!("Provenance App {}", env!("CARGO_PKG_VERSION"));
    info!(
        "State sizes\ncomm: {}\nstates: {}\nhostio: {}",
        core::mem::size_of::<io::Comm>(),
        core::mem::size_of::<APDUsFuture>(),
        core::mem::size_of::<HostIOState>()
    );

    let // Draw some 'welcome' screen
        menu = |states : core::cell::Ref<'_, Option<APDUsFuture>>, idle : & mut RootMenu<2>, busy : & mut RootMenu<2>| {
            match states.is_none() {
                true => idle.show(),
                _ => busy.show(),
            }
        };

    noinline(|| menu(states.borrow(), &mut idle_menu, &mut busy_menu));
    loop {
        // Wait for either a specific button push to exit the app
        // or an APDU command
        let evt = comm.borrow_mut().next_event();
        match evt {
            io::Event::Command(ins) => {
                trace!("Command received");
                let poll_rv = poll_apdu_handlers(
                    PinMut::as_mut(&mut states.borrow_mut()),
                    ins,
                    host_io,
                    FutureTrampolineRunner,
                    handle_apdu_async,
                );
                match poll_rv {
                    Ok(()) => {
                        trace!("APDU accepted; sending response");
                        comm.borrow_mut().reply_ok();
                        trace!("Replied");
                    }
                    Err(sw) => comm.borrow_mut().reply(sw),
                };
                noinline(|| menu(states.borrow(), &mut idle_menu, &mut busy_menu));
                trace!("Command done");
            }
            io::Event::Button(btn) => {
                trace!("Button received");
                match states.borrow().is_none() {
                    true => match noinline(|| idle_menu.update(btn)) {
                        Some(1) => {
                            info!("Exiting app at user direction via root menu");
                            nanos_sdk::exit_app(0)
                        }
                        _ => (),
                    },
                    false => match noinline(|| busy_menu.update(btn)) {
                        Some(1) => {
                            info!("Resetting at user direction via busy menu");
                            noinline(|| PinMut::as_mut(&mut states.borrow_mut()).set(None))
                        }
                        _ => (),
                    },
                };
                menu(states.borrow(), &mut idle_menu, &mut busy_menu);
                trace!("Button done");
            }
            io::Event::Ticker => {
                //trace!("Ignoring ticker event");
            }
        }
    }
}

/*
use nanos_sdk::io::Reply;

#[inline(never)]
fn handle_apdu<'a: 'b, 'b>(
    io: HostIO,
    ins: Ins,
    state: &'b mut Pin<&'a mut ParsersState<'a>>,
) -> Result<(), Reply> {
    let comm = io.get_comm();
    if comm?.rx == 0 {
        return Err(io::StatusWords::NothingReceived.into());
    }

    match ins {
        Ins::GetVersion => {}
        Ins::GetPubkey => poll_apdu_handler(state, io, &mut FutureTrampolineRunner, GetAddress)?,
        Ins::Sign => {
            trace!("Handling sign");
            poll_apdu_handler(state, io, &mut FutureTrampolineRunner, Sign)?
        }
        Ins::GetVersionStr => {}
        Ins::Exit => nanos_sdk::exit_app(0),
    }
    Ok(())
}
*/
