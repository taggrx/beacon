use std::cell::RefCell;

use candid::Principal;
use ic_cdk::{
    api::{
        call::{arg_data_raw, reply_raw},
        stable,
    },
    caller, spawn,
};
use ic_cdk_macros::*;
use ic_cdk_timers::{set_timer, set_timer_interval};
use ic_ledger_types::{Memo, Tokens};
use icp::{account_balance_of_principal, principal_to_subaccount};
use icrc1::Value;
use order_book::{State, TokenId};
use xdr_rate::get_xdr_in_e8s;

mod assets;
mod icp;
mod icrc1;
mod order_book;
mod xdr_rate;

thread_local! {
    static STATE: RefCell<State> = Default::default();
}

fn read<F, R>(f: F) -> R
where
    F: FnOnce(&State) -> R,
{
    STATE.with(|cell| f(&cell.borrow()))
}

fn mutate<F, R>(f: F) -> R
where
    F: FnOnce(&mut State) -> R,
{
    STATE.with(|cell| f(&mut cell.borrow_mut()))
}

#[export_name = "canister_query token"]
fn token() {
    let id: Principal = parse(&arg_data_raw());
    read(|state| reply(state.get_token(id)));
}

#[export_name = "canister_query subaccount"]
fn subaccount() {
    reply(icp::user_account(caller()).to_string());
}

#[export_name = "canister_query params"]
fn params() {
    read(|state| reply(state.e8s_per_xdr));
}

#[update]
fn set_revenue_account(new_address: Principal) {
    mutate(|state| {
        if state.revenue_account.is_none() || state.revenue_account == Some(caller()) {
            state.revenue_account = Some(new_address);
        }
    })
}

#[export_name = "canister_update list_token"]
fn list_token() {
    spawn(async {
        let token: TokenId = parse(&arg_data_raw());
        reply(list_token_core(token).await)
    });
}

fn parse<'a, T: serde::Deserialize<'a>>(bytes: &'a [u8]) -> T {
    serde_json::from_slice(bytes).expect("couldn't parse the input")
}

fn reply<T: serde::Serialize>(data: T) {
    reply_raw(serde_json::json!(data).to_string().as_bytes());
}

#[init]
fn init() {
    assets::load();
}

#[pre_upgrade]
fn pre_upgrade() {
    let buffer: Vec<u8> =
        read(|state| serde_cbor::to_vec(state).expect("couldn't serialize the environment"));
    let len = buffer.len() + 4;
    if len > (stable::stable_size() << 16) as usize
        && stable::stable_grow((len >> 16) as u32 + 1).is_err()
    {
        panic!("couldn't grow memory");
    }
    stable::stable_write(0, (buffer.len() as u32).to_be_bytes().as_ref());
    stable::stable_write(4, &buffer);
}

#[post_upgrade]
fn post_upgrade() {
    let bytes = stable::stable_bytes();
    let mut len_bytes: [u8; 4] = Default::default();
    len_bytes.copy_from_slice(&bytes[..4]);
    let len = u32::from_be_bytes(len_bytes) as usize;

    STATE.with(|cell| {
        cell.replace(
            serde_cbor::from_slice(&bytes[4..4 + len]).expect("couldn't deserialize state"),
        )
    });
    assets::load();
    let fetch_rate = || {
        spawn(async {
            if let Ok(e8s) = get_xdr_in_e8s().await {
                mutate(|state| state.e8s_per_xdr = e8s);
            }
        })
    };
    set_timer(std::time::Duration::from_secs(1), fetch_rate);
    set_timer_interval(std::time::Duration::from_secs(60 * 60), fetch_rate);
}

async fn list_token_core(token: TokenId) -> Result<(), String> {
    let balance = account_balance_of_principal(caller()).await;
    let listing_price = Tokens::from_e8s(read(|state| state.e8s_per_xdr * 100));
    if balance < listing_price {
        return Err(format!(
            "Balance too low! Expected: {}, found: {}.",
            listing_price, balance
        ));
    }

    let metadata = icrc1::metadata(token)
        .await
        .map_err(|err| format!("couldn't fetch metadata: {}", err))?;

    match (
        metadata.get("icrc1:symbol"),
        metadata.get("icrc1:fee"),
        metadata.get("icrc1:decimals"),
        metadata.get("icrc1:logo"),
    ) {
        (Some(Value::Text(symbol)), Some(Value::Nat(fee)), Some(Value::Nat(decimals)), logo) => {
            mutate(|state| {
                state.add_token(
                    token,
                    symbol.clone(),
                    *fee,
                    *decimals as u32,
                    match logo {
                        Some(Value::Text(hex)) => Some(hex.clone()),
                        _ => None,
                    },
                )
            })
        }
        (symbol, fee, decimals, _) => {
            return Err(format!(
                "one of the required values missing: symbol={:?}, fee={:?}, decimals={:?}",
                symbol, fee, decimals
            ));
        }
    }

    icp::transfer(
        icp::revenue_account(),
        balance,
        Memo(0),
        Some(principal_to_subaccount(&caller())),
    )
    .await
    .map_err(|err| format!("transfer failed: {}", err))?;

    Ok(())
}
