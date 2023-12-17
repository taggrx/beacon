use icrc1::Account;
use serde::Serialize;
use std::cell::RefCell;
use std::time::Duration;

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
use ic_ledger_types::{Tokens as ICP, DEFAULT_FEE, MAINNET_LEDGER_CANISTER_ID};
use order_book::{Order, OrderType, State, TokenId, Tokens, TX_FEE};

mod assets;
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
    read(|state| reply(state.token(id)));
}

#[export_name = "canister_query tokens"]
fn tokens() {
    read(|state| reply(state.tokens()));
}

#[derive(Serialize)]
struct Data {
    e8s_per_xdr: u64,
    fee: u128,
}

#[export_name = "canister_query params"]
fn params() {
    read(|state| {
        reply(Data {
            e8s_per_xdr: state.e8s_per_xdr,
            fee: TX_FEE,
        })
    })
}

#[export_name = "canister_query token_balances"]
fn token_balances() {
    reply(read(|state| state.token_balances(caller())));
}

#[query]
fn orders(token: TokenId, order_type: OrderType) -> Vec<Order> {
    read(|state| state.orders(token, order_type).cloned().collect())
}

#[update]
fn set_revenue_account(new_address: Principal) {
    mutate(|state| {
        if state.revenue_account.is_none() || state.revenue_account == Some(caller()) {
            state.revenue_account = Some(new_address);
        }
    })
}

#[update]
async fn close_order(
    token: TokenId,
    order_type: OrderType,
    amount: u128,
    price: Tokens,
) -> Result<(), String> {
    mutate(|state| state.close_order(caller(), token, amount, price, order_type))
}

#[update]
async fn trade(
    token: TokenId,
    amount: u128,
    price: Tokens,
    order_type: OrderType,
) -> Result<(u128, bool), String> {
    let pool_token = if order_type.buy() {
        MAINNET_LEDGER_CANISTER_ID
    } else {
        token
    };
    let user = caller();
    let user_account = icrc1::user_account(user);

    let required_liquidity = if order_type.buy() {
        amount * price
    } else {
        amount
    };

    // lock liquidity needed
    icrc1::transfer(
        pool_token,
        user_account.subaccount,
        icrc1::main_account(),
        required_liquidity,
    )
    .await
    .map_err(|err| format!("transfer failed: {}", err))?;

    mutate(|state| {
        state.add_liquidity(user, pool_token, required_liquidity)?;

        // match existing orders
        let filled = state
            .trade(
                order_type,
                user,
                token,
                amount,
                Some(price),
                ic_cdk::api::time(),
            )
            .expect("trade failed");

        // create a rest order if the original was not filled and this was a limit order
        Ok((
            filled,
            if filled < amount && price > 0 {
                state
                    .create_order(
                        user,
                        token,
                        amount.saturating_sub(filled),
                        price,
                        order_type,
                    )
                    .expect("order creation failed");
                true
            } else {
                false
            },
        ))
    })
}

#[update]
async fn withdraw(token_id: Principal) -> Result<u128, String> {
    let user = caller();
    let balance = mutate(|state| state.withdraw_liquidity(user, token_id))?;
    let fee = read(|state| state.token(token_id))?.fee;
    let amount = balance - fee;
    icrc1::transfer(
        token_id,
        None,
        Account {
            owner: user,
            subaccount: None,
        },
        amount,
    )
    .await
    .map_err(|err| format!("transfer failed: {}", err))
    .map(|_| balance)
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

#[init]
fn init() {
    kickstart();
    set_timer(Duration::from_millis(1), || {
        spawn(async {
            register_token(MAINNET_LEDGER_CANISTER_ID)
                .await
                .expect("couldn't register ICP");
        })
    });
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

    kickstart();
}

async fn list_token_core(token: TokenId) -> Result<(), String> {
    let user_account = icrc1::user_account(caller());
    let balance = icrc1::balance_of(MAINNET_LEDGER_CANISTER_ID, &user_account).await?;
    let listing_price = read(|state| state.e8s_per_xdr * 100);
    if balance < listing_price as u128 {
        return Err(format!(
            "Balance too low! Expected: {}, found: {}.",
            listing_price, balance
        ));
    }

    register_token(token).await?;

    icrc1::transfer(
        MAINNET_LEDGER_CANISTER_ID,
        user_account.subaccount,
        icrc1::main_account(),
        // we subtract fees twice, because the user paid once already when deploying to their
        // subaccount
        (ICP::from_e8s(listing_price) - DEFAULT_FEE - DEFAULT_FEE).e8s() as u128,
    )
    .await
    .map_err(|err| format!("transfer failed: {}", err))?;

    Ok(())
}

async fn register_token(token: TokenId) -> Result<(), String> {
    let metadata = icrc1::metadata(token)
        .await
        .map_err(|err| format!("couldn't fetch metadata: {}", err))?;

    use icrc1::Value;
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
                    *fee as u128,
                    *decimals as u32,
                    match logo {
                        Some(Value::Text(hex)) => Some(hex.clone()),
                        _ => None,
                    },
                );
                Ok(())
            })
        }
        (symbol, fee, decimals, _) => Err(format!(
            "one of the required values missing: symbol={:?}, fee={:?}, decimals={:?}",
            symbol, fee, decimals
        )),
    }
}
