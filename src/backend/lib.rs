use icrc1::Account;
use serde::Serialize;
use std::collections::BTreeMap;
use std::time::Duration;
use std::{cell::RefCell, collections::VecDeque};

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
mod memory;
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

#[cfg(feature = "dev")]
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
    set_timer_interval(Duration::from_secs(15 * 60), fetch_rate);
}

fn stable_to_heap_core() {
    STATE.with(|cell| cell.replace(memory::stable_to_heap()));
    kickstart();
}
