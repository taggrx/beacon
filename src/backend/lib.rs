use icrc1::Account;
use serde::Serialize;
use std::collections::BTreeMap;
use std::time::Duration;
use std::{cell::RefCell, collections::VecDeque};

use candid::Principal;
use ic_cdk::{
    api::{call::reply_raw, stable},
    caller, spawn,
};
use ic_cdk_macros::*;
use ic_cdk_timers::{set_timer, set_timer_interval};
use ic_ledger_types::{Tokens as ICP, DEFAULT_FEE};
use order_book::{Order, OrderType, State, Timestamp, TokenId, Tokens, PAYMENT_TOKEN_ID, TX_FEE};

mod assets;
mod icrc1;
mod order_book;
mod queries;
mod updates;
mod xdr_rate;

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

// This method deposits liquidity from user's subaccount into the token pools.
//
// It first checks, if there's any pending liquidity on users' subaccount. If yes, it moves the
// liquidity to the corresponding token pool, and makes a corresponding accounting of the user's
// share.
//
// If the balance is smaller than the fee, the function does nothing.
async fn deposit_liquidity(user: Principal, token: TokenId) -> Result<(), String> {
    let user_account = icrc1::user_account(user);
    let wallet_balance = icrc1::balance_of(token, &user_account)
        .await?
        // subtract fee becasue this funds will be moved to BEACON pool
        .checked_sub(read(|state| state.token(token))?.fee)
        .unwrap_or_default();

    // if the balance is above 0, move everything from the wallet to BEACON
    if wallet_balance > 0 {
        icrc1::transfer(
            token,
            user_account.subaccount,
            icrc1::main_account(),
            wallet_balance,
        )
        .await
        .map_err(|err| {
            let error = format!("deposit transfer failed: {}", err);
            mutate(|state| state.log(error.clone()));
            error
        })?;
        mutate_with_invarant_check(
            |state| state.add_liquidity(user, token, wallet_balance),
            Some((token, wallet_balance as i128)),
        )?;
    }
    Ok(())
}
