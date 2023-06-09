use crate::crypto_helpers::{format_signature, get_pkh, get_pubkey};
use crate::interface::*;
use crate::settings::*;
use crate::utils::*;

pub use crate::proto::cosmos::bank::v1beta1::*;
pub use crate::proto::cosmos::base::v1beta1::*;
pub use crate::proto::cosmos::gov::v1beta1::*;
pub use crate::proto::cosmos::staking::v1beta1::*;
pub use crate::proto::cosmos::tx::v1beta1::*;

use alamgu_async_block::*;
use arrayvec::{ArrayString, ArrayVec};
use core::fmt::Write;

use ledger_crypto_helpers::hasher::{Base64Hash, Hasher, SHA256};
use ledger_log::trace;
use ledger_parser_combinators::any_of;
use ledger_parser_combinators::async_parser::*;
use ledger_parser_combinators::interp::*;
use ledger_parser_combinators::protobufs::async_parser::*;
use ledger_parser_combinators::protobufs::schema::Bytes;
use ledger_parser_combinators::protobufs::schema::ProtobufWireFormat;
use ledger_prompts_ui::{final_accept_prompt, ScrollerError};

use core::convert::TryFrom;
use core::future::Future;

use nanos_sdk::ecc::*;

pub type BipParserImplT =
    impl AsyncParser<Bip32Key, ByteStream> + HasOutput<Bip32Key, Output = ArrayVec<u32, 10>>;
pub const BIP_PATH_PARSER: BipParserImplT = SubInterp(DefaultInterp);

// Need a path of length 5, as make_bip32_path panics with smaller paths
pub const BIP32_PREFIX: [u32; 3] = nanos_sdk::ecc::make_bip32_path(b"m/44'/505'/0'");

pub async fn get_address_apdu(io: HostIO, prompt: bool) {
    let input = match io.get_params::<1>() {
        Some(v) => v,
        None => reject().await,
    };

    let path = BIP_PATH_PARSER.parse(&mut input[0].clone()).await;

    if !path.starts_with(&BIP32_PREFIX[0..2]) {
        reject::<()>().await;
    }

    let prompt_fn = || {
        let pubkey = get_pubkey(&path).ok()?; // Secp256k1::derive_from_path(&path).public_key().ok()?;
        let pkh = get_pkh(&pubkey).ok()?;
        Some((pubkey, pkh))
    };

    if let Some((pubkey, pkh)) = prompt_fn() {
        if prompt {
            scroller("Provide Public Key", |_w| Ok(()));
            scroller_paginated("Address", |w| Ok(write!(w, "{pkh}")?));
            final_accept_prompt(&[]);
        }

        let mut rv = ArrayVec::<u8, 128>::new();
        let mut add_to_rv_fn = || -> Option<()> {
            rv.try_push(u8::try_from(pubkey.len()).ok()?).ok()?;
            rv.try_extend_from_slice(&pubkey).ok()?;

            let mut temp_fmt = ArrayString::<50>::new();
            write!(temp_fmt, "{}", pkh).ok()?;

            rv.try_push(u8::try_from(temp_fmt.as_bytes().len()).ok()?)
                .ok()?;
            rv.try_extend_from_slice(temp_fmt.as_bytes()).ok()
        };

        if add_to_rv_fn().is_none() {
            reject::<()>().await;
        };

        io.result_final(&rv).await;
    } else {
        reject::<()>().await;
    }
}

// We'd like this to be just a const fn, but the resulting closure rather than function pointer seems to crash the app.
macro_rules! show_string {
    {$do_prompt: expr, $n: literal, $msg:literal}
    => {
        Action(
            Buffer::<$n>, |pkh: ArrayVec<u8, $n>| {
                if $do_prompt {
                    scroller($msg, |w| Ok(write!(w, "{}", core::str::from_utf8(pkh.as_slice())?)?))
                } else {
                    Some(())
                }
            }
        )
    };
    {ifnonempty, $do_prompt: expr, $n: literal, $msg:literal}
    => {
        Action(
            Buffer::<$n>, |pkh: ArrayVec<u8, $n>| {
                if pkh.is_empty() { Some(()) } else {
                    if $do_prompt {
                        scroller($msg, |w| Ok(write!(w, "{}", core::str::from_utf8(pkh.as_slice())?)?))
                    } else {
                        Some(())
                    }
                }
            }
        )
    }
}

const fn show_coin<BS: 'static + Readable + ReadableLength + Clone, const PROMPT: bool>(
) -> impl LengthDelimitedParser<Coin, BS> {
    Action(
        CoinUnorderedInterp {
            field_denom: Buffer::<20>,
            field_amount: Buffer::<64>,
        },
        move |CoinValue {
                  field_denom,
                  field_amount,
              }: CoinValue<Option<ArrayVec<u8, 20>>, Option<ArrayVec<u8, 64>>>| {
            if PROMPT {
                show_amount_in_decimals(true, "Amount", &field_amount?, &field_denom?)
            } else {
                Some(())
            }
        },
    )
}

// Handle conversion to decimals, but only if showing nhash
fn show_amount_in_decimals(
    only_hash: bool,
    title: &str,
    amount: &ArrayVec<u8, 64>,
    denom: &ArrayVec<u8, 20>,
) -> Option<()> {
    if denom.as_slice() == b"nhash" {
        scroller(title, |w| {
            let x = get_amount_in_decimals(amount).map_err(|_| ScrollerError)?;
            write!(w, "HASH {}", core::str::from_utf8(&x)?).map_err(|_| ScrollerError)
        })
    } else {
        if only_hash {
            None
        } else {
            scroller(title, |w| {
                write!(
                    w,
                    "{} {}",
                    core::str::from_utf8(&amount.as_slice())?,
                    core::str::from_utf8(&denom.as_slice())?
                )
                .map_err(|_| ScrollerError)
            })
        }
    }
}

// "Divides" the amount by 1000000000
// Converts the input string in the following manner
// 1 -> 0.000000001
// 10 -> 0.00000001
// 11 -> 0.000000011
// 1000000000 -> 1.0
// 10000000000 -> 10.0
// 10010000000 -> 10.01
// 010010000000 -> 10.01
fn get_amount_in_decimals(amount: &ArrayVec<u8, 64>) -> Result<ArrayVec<u8, 64>, ()> {
    let mut found_first_non_zero = false;
    let mut start_ix = 0;
    let mut last_non_zero_ix = 0;
    // check the amount for any invalid chars and get its length
    for (ix, c) in amount.as_ref().iter().enumerate() {
        if c < &b'0' || c > &b'9' {
            return Err(());
        }
        if c != &b'0' {
            last_non_zero_ix = ix;
        }
        if !found_first_non_zero {
            if c == &b'0' {
                // Highly unlikely to hit this, but skip any leading zeroes
                continue;
            }
            start_ix = ix;
            found_first_non_zero = true;
        }
    }

    let mut dec_value: ArrayVec<u8, 64> = ArrayVec::new();
    let amt_len = amount.len() - start_ix;
    let chars_after_decimal = 9;
    if amt_len > chars_after_decimal {
        // value is more than 1
        dec_value
            .try_extend_from_slice(&amount.as_ref()[start_ix..(amount.len() - chars_after_decimal)])
            .map_err(|_| ())?;
        dec_value.try_push(b'.').map_err(|_| ())?;
        if amount.len() - chars_after_decimal <= last_non_zero_ix {
            // there is non-zero decimal value
            dec_value
                .try_extend_from_slice(
                    &amount.as_ref()[amount.len() - chars_after_decimal..(last_non_zero_ix + 1)],
                )
                .map_err(|_| ())?;
        } else {
            // add a zero at the end always "xyz.0"
            dec_value.try_push(b'0').map_err(|_| ())?;
        }
    } else {
        // value is less than 1
        dec_value.try_push(b'0').map_err(|_| ())?;
        dec_value.try_push(b'.').map_err(|_| ())?;
        for _i in 0..(chars_after_decimal - amt_len) {
            dec_value.try_push(b'0').map_err(|_| ())?;
        }
        dec_value
            .try_extend_from_slice(&amount.as_ref()[start_ix..(last_non_zero_ix + 1)])
            .map_err(|_| ())?;
    }
    Ok(dec_value)
}

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

const fn txn_messages_parser<const PROMPT: bool>(
) -> impl LengthDelimitedParser<Transaction, LengthTrack<ByteStream>> + HasOutput<Transaction> {
    SignDocUnorderedInterp {
        field_body_bytes: BytesAsMessage(
            TxBody,
            TxBodyUnorderedInterp {
                field_messages: MessagesInterp {
                    default: RawAnyInterp {
                        field_type_url: Preaction(
                            || {
                                // scroller("Unknown", |w| Ok(write!(w, "Message")?))
                                // Always reject unknown messages
                                None
                            },
                            show_string!(PROMPT, 50, "Type URL"),
                        ),
                        field_value: DropInterp,
                    },
                    send: (Action(
                        MsgSendUnorderedInterp {
                            field_from_address: Buffer::<50>,
                            field_to_address: Buffer::<50>,
                            field_amount: CoinUnorderedInterp {
                                field_denom: Buffer::<20>,
                                field_amount: Buffer::<64>,
                            },
                        },
                        |o: MsgSendValue<
                            Option<ArrayVec<u8, 50>>,
                            Option<ArrayVec<u8, 50>>,
                            Option<CoinValue<Option<ArrayVec<u8, 20>>, Option<ArrayVec<u8, 64>>>>,
                        >|
                         -> Option<()> {
                            if PROMPT {
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
                                        o.field_to_address
                                            .as_ref()
                                            .ok_or(ScrollerError)?
                                            .as_slice(),
                                    )?;
                                    write!(w, "{x}").map_err(|_| ScrollerError) // TODO don't map_err
                                });
                                show_amount_in_decimals(
                                    true,
                                    "Amount",
                                    o.field_amount.as_ref()?.field_amount.as_ref()?,
                                    o.field_amount.as_ref()?.field_denom.as_ref()?,
                                )
                            } else {
                                Some(())
                            }
                        },
                    )),
                    delegate: (Preaction(
                        || {
                            if PROMPT {
                                scroller("Delegate", |_| Ok(()))
                            } else {
                                Some(())
                            }
                        },
                        MsgDelegateInterp {
                            field_amount: show_coin::<_, PROMPT>(),
                            field_delegator_address: show_string!(PROMPT, 50, "Delegator Address"),
                            field_validator_address: show_string!(PROMPT, 50, "Validator Address"),
                        },
                    )),
                    undelegate: (Preaction(
                        || {
                            if PROMPT {
                                scroller("Undelegate", |_| Ok(()))
                            } else {
                                Some(())
                            }
                        },
                        MsgUndelegateInterp {
                            field_amount: show_coin::<_, PROMPT>(),
                            field_delegator_address: show_string!(PROMPT, 50, "Delegator Address"),
                            field_validator_address: show_string!(PROMPT, 50, "Validator Address"),
                        },
                    )),
                    deposit: (MsgDepositInterp {
                        field_amount: show_coin::<_, PROMPT>(),
                        field_depositor: show_string!(PROMPT, 50, "Depositor Address"),
                        field_proposal_id: Action(DefaultInterp, |value: u64| {
                            if PROMPT {
                                scroller("Proposal ID", |w| Ok(write!(w, "{}", value)?))
                            } else {
                                Some(())
                            }
                        }),
                    }),
                },
                field_memo: DropInterp,
                field_timeout_height: DropInterp,
                field_extension_options: DropInterp,
                field_non_critical_extension_options: DropInterp,
            },
        ),
        field_auth_info_bytes: BytesAsMessage(
            AuthInfo,
            AuthInfoUnorderedInterp {
                field_signer_infos: DropInterp,
                field_tip: DropInterp,
                field_fee: Action(
                    FeeUnorderedInterp {
                        field_amount: CoinUnorderedInterp {
                            field_denom: Buffer::<20>,
                            field_amount: Buffer::<64>,
                        },
                        field_gas_limit: DefaultInterp,
                        field_payer: DropInterp,
                        field_granter: DropInterp,
                    },
                    |o: FeeValue<
                        Option<CoinValue<Option<ArrayVec<u8, 20>>, Option<ArrayVec<u8, 64>>>>,
                        Option<u64>,
                        Option<()>,
                        Option<()>,
                    >|
                     -> Option<()> {
                        if PROMPT {
                            show_amount_in_decimals(
                                true,
                                "Fee",
                                o.field_amount.as_ref()?.field_amount.as_ref()?,
                                o.field_amount.as_ref()?.field_denom.as_ref()?,
                            )?;
                            scroller("Gas Limit", |w| {
                                Ok(write!(
                                    w,
                                    "{}",
                                    o.field_gas_limit.as_ref().ok_or(ScrollerError)?
                                )?)
                            })
                        } else {
                            Some(())
                        }
                    },
                ),
            },
        ),
        field_chain_id: DropInterp,
        field_account_number: DropInterp,
    }
}

const fn hasher_parser(
) -> impl LengthDelimitedParser<Bytes, ByteStream> + HasOutput<Bytes, Output = (SHA256, Option<()>)>
{
    ObserveBytes(SHA256::new, SHA256::update, DropInterp)
}

any_of! {
MessagesInterp {
    Send: MsgSend = b"/cosmos.bank.v1beta1.MsgSend",
    Delegate: MsgDelegate = b"/cosmos.staking.v1beta1.MsgDelegate",
    Undelegate: MsgUndelegate = b"/cosmos.staking.v1beta1.MsgUndelegate",
    Deposit: MsgDeposit = b"/cosmos.gov.v1.MsgDeposit"
}
}

pub async fn sign_apdu(io: HostIO, settings: Settings) {
    let mut input = match io.get_params::<2>() {
        Some(v) => v,
        None => reject().await,
    };

    let length = usize::from_le_bytes(input[0].read().await);

    let path = NoinlineFut((|mut bs: ByteStream| async move {
        {
            BIP_PATH_PARSER.parse(&mut bs).await
        }
    })(input[1].clone()))
    .await;

    if !path.starts_with(&BIP32_PREFIX[0..2]) {
        reject::<()>().await;
    }

    let known_txn = NoinlineFut((|bs: ByteStream| async move {
        {
            let mut txn = LengthTrack(bs, 0);
            TryFuture(txn_messages_parser::<false>().parse(&mut txn, length))
                .await
                .is_some()
        }
    })(input[0].clone()))
    .await;

    if known_txn {
        NoinlineFut((|bs: ByteStream| async move {
            {
                let mut txn = LengthTrack(bs, 0);
                txn_messages_parser::<true>().parse(&mut txn, length).await;
            }
        })(input[0].clone()))
        .await;
    } else if settings.get() == 0 {
        scroller("WARNING", |w| {
            Ok(write!(
                w,
                "Transaction not recognized, enable blind signing to sign unknown transactions"
            )?)
        });
        reject::<()>().await;
    } else if scroller("WARNING", |w| Ok(write!(w, "Transaction not recognized")?)).is_none() {
        reject::<()>().await;
    }

    NoinlineFut(async move {
        let hash: Base64Hash<32>;

        {
            let mut txn = input[0].clone();
            hash = hasher_parser().parse(&mut txn, length).await.0.finalize();
            trace!("Hashed txn");
        }

        if !known_txn {
            if scroller("Transaction Hash", |w| Ok(write!(w, "{}", hash)?)).is_none() {
                reject::<()>().await;
            };
        }

        if known_txn {
            if final_accept_prompt(&[]).is_none() {
                reject::<()>().await;
            }
        } else {
            if final_accept_prompt(&["Blind Sign Transaction?"]).is_none() {
                reject::<()>().await;
            }
        }

        let sk = Secp256k1::derive_from_path(&path);
        if let Some(v) = sk.deterministic_sign(&hash.0[..]).ok() {
            if let Some(sig) = { format_signature(&v) } {
                io.result_final(&sig).await;
            } else {
                reject::<()>().await;
            }
        } else {
            reject::<()>().await;
        }
    })
    .await;
}

pub type APDUsFuture = impl Future<Output = ()>;

#[inline(never)]
pub fn handle_apdu_async(io: HostIO, ins: Ins, settings: Settings) -> APDUsFuture {
    trace!("Constructing future");
    async move {
        trace!("Dispatching");
        match ins {
            Ins::GetVersion => {
                const APP_NAME: &str = "Provenance";
                let mut rv = ArrayVec::<u8, 220>::new();
                let _ = rv.try_push(env!("CARGO_PKG_VERSION_MAJOR").parse().unwrap());
                let _ = rv.try_push(env!("CARGO_PKG_VERSION_MINOR").parse().unwrap());
                let _ = rv.try_push(env!("CARGO_PKG_VERSION_PATCH").parse().unwrap());
                let _ = rv.try_extend_from_slice(APP_NAME.as_bytes());
                io.result_final(&rv).await;
            }
            Ins::VerifyAddress => {
                NoinlineFut(get_address_apdu(io, true)).await;
            }
            Ins::GetPubkey => {
                NoinlineFut(get_address_apdu(io, false)).await;
            }
            Ins::Sign => {
                trace!("Handling sign");
                NoinlineFut(sign_apdu(io, settings)).await;
            }
            Ins::GetVersionStr => {}
            Ins::Exit => nanos_sdk::exit_app(0),
        }
    }
}
