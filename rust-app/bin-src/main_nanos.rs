use core::borrow::Borrow;
use core::cell::RefCell;
use core::pin::Pin;

use alamgu_async_block::*;
use provenance::implementation::*;
use provenance::interface::*;
use provenance::menu::*;
use provenance::settings::*;

use nanos_sdk::io;
nanos_sdk::set_panic!(nanos_sdk::exiting_panic);

use provenance::*;

use ledger_prompts_ui::{handle_menu_button_event, show_menu};

static mut COMM_CELL: Option<RefCell<io::Comm>> = None;

static mut HOST_IO_STATE: Option<RefCell<HostIOState>> = None;
static mut STATES_BACKING: ParsersState<'static> = ParsersState::NoState;

#[inline(never)]
unsafe fn initialize() {
    STATES_BACKING = ParsersState::NoState;
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
    let mut states = unsafe { Pin::new_unchecked(&mut STATES_BACKING) };

    let mut idle_menu = IdleMenuWithSettings {
        idle_menu: IdleMenu::AppMain,
        settings: Settings::default(),
    };
    let mut busy_menu = BusyMenu::Working;

    info!("Provenance App {}", env!("CARGO_PKG_VERSION"));
    info!(
        "State sizes\ncomm: {}\nstates: {}\nhostio: {}",
        core::mem::size_of::<io::Comm>(),
        core::mem::size_of::<ParsersState>(),
        core::mem::size_of::<HostIOState>()
    );

    let menu = |states: &ParsersState, idle: &IdleMenuWithSettings, busy: &BusyMenu| match states {
        ParsersState::NoState => show_menu(idle),
        _ => show_menu(busy),
    };

    // Draw some 'welcome' screen
    noinline(|| menu(states.borrow(), &idle_menu, &busy_menu));
    loop {
        // Wait for either a specific button push to exit the app
        // or an APDU command
        let evt = comm.borrow_mut().next_event::<Ins>();
        match evt {
            io::Event::Command(ins) => {
                trace!("Command received");
                match handle_apdu(comm, host_io, ins, &mut states) {
                    Ok(()) => {
                        trace!("APDU accepted; sending response");
                        comm.borrow_mut().reply_ok();
                        trace!("Replied");
                    }
                    Err(sw) => comm.borrow_mut().reply(sw),
                };
                // Reset BusyMenu if we are done handling APDU
                match states.as_ref().get_ref() {
                    ParsersState::NoState => busy_menu = BusyMenu::Working,
                    _ => {}
                };
                noinline(|| menu(states.borrow(), &idle_menu, &busy_menu));
                trace!("Command done");
            }
            io::Event::Button(btn) => {
                trace!("Button received");
                match states.is_no_state() {
                    true => match noinline(|| handle_menu_button_event(&mut idle_menu, btn)) {
                        Some(DoExitApp) => {
                            info!("Exiting app at user direction via root menu");
                            nanos_sdk::exit_app(0)
                        }
                        _ => (),
                    },
                    false => match noinline(|| handle_menu_button_event(&mut busy_menu, btn)) {
                        Some(DoCancel) => {
                            info!("Resetting at user direction via busy menu");
                            noinline(|| reset_parsers_state(&mut states))
                        }
                        _ => (),
                    },
                };
                menu(states.borrow(), &idle_menu, &busy_menu);
                trace!("Button done");
            }
            io::Event::Ticker => {
                //trace!("Ignoring ticker event");
            }
        }
    }
}

use nanos_sdk::io::Reply;

#[inline(never)]
fn handle_apdu<'a: 'b, 'b>(
    comm: &RefCell<io::Comm>,
    io: HostIO,
    ins: Ins,
    state: &'b mut Pin<&'a mut ParsersState<'a>>,
) -> Result<(), Reply> {
    let comm_ = io.get_comm();
    if comm_?.rx == 0 {
        return Err(io::StatusWords::NothingReceived.into());
    }

    match ins {
        Ins::GetVersion => {
            comm.borrow_mut()
                .append(&[LedgerToHostCmd::ResultFinal as u8]);
            comm.borrow_mut().append(&[
                env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap(),
                env!("CARGO_PKG_VERSION_MINOR").parse().unwrap(),
                env!("CARGO_PKG_VERSION_PATCH").parse().unwrap(),
            ]);
            comm.borrow_mut().append(b"Provenance");
        }
        Ins::VerifyAddress => {
            poll_apdu_handler(state, io, &mut FutureTrampolineRunner, GetAddress::<true>)?
        }
        Ins::GetPubkey => {
            poll_apdu_handler(state, io, &mut FutureTrampolineRunner, GetAddress::<false>)?
        }
        Ins::Sign => {
            trace!("Handling sign");
            poll_apdu_handler(state, io, &mut FutureTrampolineRunner, Sign)?
        }
        Ins::GetVersionStr => {}
        Ins::Exit => nanos_sdk::exit_app(0),
    }
    Ok(())
}
