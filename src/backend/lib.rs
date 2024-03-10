use ic_cdk::api::stable::{stable64_grow, stable64_read, stable64_size, stable64_write};
use icrc1::Account;
use serde::Serialize;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::time::Duration;

use candid::Principal;
use ic_cdk::{api::call::reply_raw, caller, spawn};
use ic_cdk_macros::*;
use ic_cdk_timers::{set_timer, set_timer_interval};
use ic_ledger_types::{Tokens as ICP, DEFAULT_FEE};
use order_book::{Order, OrderType, State, Timestamp, TokenId, Tokens, PAYMENT_TOKEN_ID, TX_FEE};

mod assets;
#[cfg(feature = "dev")]
mod dev_helpers;
mod icrc1;
mod order_book;
mod queries;
mod updates;
mod xdr_rate;

const BACKUP_PAGE_SIZE: u32 = 1024 * 1024;
pub const MINUTE: u64 = 60000000000_u64;
pub const HOUR: u64 = 60 * MINUTE;
pub const DAY: u64 = 24 * HOUR;

thread_local! {
    static STATE: RefCell<State> = Default::default();
}

fn read<F, R>(f: F) -> R
where
    F: FnOnce(&State) -> R,
{
    STATE.with(|cell| f(&cell.borrow()))
}

#[cfg(any(test, feature = "dev"))]
fn unsafe_mutate<F, R>(f: F) -> R
where
    F: FnOnce(&mut State) -> R,
{
    STATE.with(|cell| f(&mut cell.borrow_mut()))
}

fn mutate<F, R>(f: F) -> R
where
    F: FnOnce(&mut State) -> R,
{
    mutate_with_invarant_check(f, None)
}

// Mutates the state and checks one invariant: that any token liquidity was changed only
// in line with the expected delta (or not changed at all, if delta is None.
fn mutate_with_invarant_check<F, R>(f: F, liquidity_delta: Option<(TokenId, i128)>) -> R
where
    F: FnOnce(&mut State) -> R,
{
    let balances_before = read(|state| state.funds_under_management());
    let result = STATE.with(|cell| f(&mut cell.borrow_mut()));
    let mut balances_after = read(|state| state.funds_under_management());
    if let Some((token_id, delta)) = liquidity_delta {
        if let Some((_, balance)) = balances_after
            .iter_mut()
            .find(|(id, _)| id == &token_id.to_string())
        {
            // if a new token was listed, remove it from the after balance if its balance is zero
            if delta == 0 && balance == &0 {
                balances_after.retain(|(id, _)| id != &token_id.to_string());
            } else {
                *balance = (*balance as i128 - delta) as u128;
            }
        }
    }
    assert_eq!(balances_before, balances_after);
    result
}

fn reply<T: serde::Serialize>(data: T) {
    reply_raw(serde_json::json!(data).to_string().as_bytes());
}

// Starts all repeating tasks.
fn kickstart() {
    assets::load();
    let fetch_rate = || {
        spawn(async {
            if let Ok(e8s) = xdr_rate::get_xdr_in_e8s().await {
                mutate(|state| state.e8s_per_xdr = e8s);
            }
        })
    };
    set_timer(Duration::from_millis(1), fetch_rate);
    set_timer_interval(Duration::from_secs(24 * 60 * 60), fetch_rate);
    set_timer_interval(Duration::from_secs(24 * 60 * 60), || {
        mutate(|state| state.clean_up(ic_cdk::api::time()));
    });
    set_timer_interval(Duration::from_secs(60 * 60), || {
        mutate(heap_to_stable);
    });
}

fn stable_to_heap_core() {
    STATE.with(|cell| cell.replace(stable_to_heap()));
}

fn parse<'a, T: serde::Deserialize<'a>>(bytes: &'a [u8]) -> T {
    serde_json::from_slice(bytes).expect("couldn't parse the input")
}

pub fn heap_to_stable(state: &mut State) {
    let offset = 16; // start of the heap
    let bytes = serde_cbor::to_vec(&state).expect("couldn't serialize the state");
    let len = bytes.len() as u64;
    if offset + len > (stable64_size() << 16) {
        stable64_grow((len >> 16) + 1).expect("couldn't grow memory");
    }
    stable64_write(offset, &bytes);
    stable64_write(0, &offset.to_be_bytes());
    stable64_write(8, &len.to_be_bytes());
}

fn stable_to_heap() -> State {
    let (offset, len) = heap_address();
    ic_cdk::println!("Reading heap from coordinates: {:?}", (offset, len));
    let mut bytes = Vec::with_capacity(len as usize);
    bytes.spare_capacity_mut();
    unsafe {
        bytes.set_len(len as usize);
    }
    stable64_read(offset, &mut bytes);
    serde_cbor::from_slice(&bytes).expect("couldn't deserialize")
}

fn heap_address() -> (u64, u64) {
    let mut offset_bytes: [u8; 8] = Default::default();
    stable64_read(0, &mut offset_bytes);
    let offset = u64::from_be_bytes(offset_bytes);
    let mut len_bytes: [u8; 8] = Default::default();
    stable64_read(8, &mut len_bytes);
    let len = u64::from_be_bytes(len_bytes);
    (offset, len)
}

use crate::assets::{HttpRequest, HttpResponse};
export_candid!();
