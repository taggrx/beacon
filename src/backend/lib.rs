use icrc1::Account;
use serde::Serialize;
use std::collections::BTreeMap;
use std::time::Duration;
use std::{cell::RefCell, collections::VecDeque};

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
use order_book::{Order, OrderType, State, Timestamp, TokenId, Tokens, TX_FEE};

mod assets;
mod icrc1;
mod order_book;
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

#[export_name = "canister_query prices"]
fn prices() {
    read(|state| {
        reply(
            state
                .tokens()
                .keys()
                .map(|token_id| {
                    (
                        token_id,
                        state
                            .order_archive
                            .get(token_id)
                            .map(|archive| archive.front().map(|order| order.price))
                            .unwrap_or_default(),
                    )
                })
                .collect::<BTreeMap<_, _>>(),
        )
    });
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

#[derive(Serialize)]
struct Stats {
    volume_day: u128,
    trades_day: u64,
    icp_locked: u128,
}

#[export_name = "canister_query stats"]
fn stats() {
    let now = ic_cdk::api::time();
    reply(read(|state| {
        let day_orders = state
            .order_archive
            .values()
            .flatten()
            .filter(|order| order.executed + DAY >= now);

        Stats {
            volume_day: day_orders
                .clone()
                .map(|order| order.amount * order.price)
                .sum(),
            trades_day: day_orders.count() as u64,
            icp_locked: state.payment_token_pool().values().sum(),
        }
    }))
}

#[query]
fn executed_orders(token: TokenId) -> VecDeque<Order> {
    let now = ic_cdk::api::time();
    read(|state| {
        state
            .order_archive
            .get(&token)
            .map(|list| {
                list.iter()
                    // take all orders from the last 3 days
                    .filter(|order| order.executed + 3 * DAY > now)
                    // but not more than 100
                    .take(100)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    })
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
    timestamp: Timestamp,
) {
    mutate(|state| state.close_order(caller(), token, amount, price, timestamp, order_type))
        .expect("couldn't close order")
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

    let balance = icrc1::balance_of(pool_token, &user_account)
        .await?
        // subtract fee becasue this funds will be moved to BEACON pool
        .checked_sub(read(|state| state.token(pool_token))?.fee)
        .unwrap_or_default();

    if balance > 0 {
        icrc1::transfer(
            pool_token,
            user_account.subaccount,
            icrc1::main_account(),
            balance,
        )
        .await
        .map_err(|err| format!("transfer failed: {}", err))?;
    }

    mutate(|state| {
        if balance > 0 {
            state.add_liquidity(user, pool_token, balance)?;
        }

        let now = ic_cdk::api::time();

        // match existing orders
        let filled = state
            .trade(
                order_type,
                user,
                token,
                amount,
                (price > 0).then_some(price),
                now,
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
                        now,
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
    let fee = read(|state| state.token(token_id))?.fee;
    let balance = mutate(|state| state.withdraw_liquidity(user, token_id))?;
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
            mutate(|state| {
                state.tokens.get_mut(&MAINNET_LEDGER_CANISTER_ID).expect("no ICP token").logo = Some("data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAACQAAAAkCAMAAADW3miqAAAAqFBMVEUAAAAPBQYCDBDpHnj8sDruHXknpNopq+IEAgLyWiQnmsvsWCMQQVY5HQ1ZJYQcdZrbHXW6H3oIIzCxeSplLRCnFlnvnjRrJIAkgrG2Tx33hy+YIXskCB0efqdMHF55Dz70bShSnsQVNkaFJIASSmF8LhJkDDRKQ5kyZH6ePhjfUyHgeCt4GFs9Ej9MMXzLqlmBH1U/CiazbFayQ5GLYSG7r258STiDf1U4y9K8AAABVUlEQVQ4y+2SyZaCMBREEwgQw6jMowqKOE9t9///WecRBOzeurSWde6pNyL00XslGTnT1MiQBgt7sa7vYg8/jVmuaiDVnnUONWN9wrVYxKZwSqY9xQzBeLuJYBbJ3mtzWLTZnE5NBHHqCpg1xOy223ifJMmeZ0nNxrKKjPLGIkG1jL41KVRNksDFqARGtDezgTI8YNbCwm5AAg8VlhVmzymBYjfOeLSzzIAQF52ssKBoTC1vk3XvUJeQAFlheBxtLAfqGw/OFyGEQ9UIwpclpyLpDxSG1VCOzuU7ULb0Wq6oqjQbGEW5szElGj9WaXro5gVGrg3YanchzIPOJcKHNJ0eMticD8ycopXaXkjiFmceDU/N0imXf/EdRTBIUFrU/JzPjyVrM6+ccRxZ4TlOTf/dvBTNZT4wPMa/9t9jd9+T99+Da9+RHb/GL3+oqiwf/+FHb9EvACscm39NBowAAAAASUVORK5CYII=".into())
            })
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
    let listing_price = read(|state| state.e8s_per_xdr * 100) - DEFAULT_FEE.e8s();
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
                    *fee,
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
