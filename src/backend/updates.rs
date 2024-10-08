use crate::order_book::{Metadata, OrderExecution};
use ic_cdk::api::time;

use super::*;

#[init]
fn init() {
    stable_grow(1).expect("stable memory intialization failed");
    kickstart();
}

#[pre_upgrade]
fn pre_upgrade() {
    mutate(heap_to_stable)
}

#[post_upgrade]
fn post_upgrade() {
    stable_to_heap_core();
    kickstart();
    read(|state| {
        ic_cdk::println!(
            "revenue_account={:?}, payment_token={}",
            state.revenue_account.map(|id| id.to_string()),
            state.payment_token_id().to_string()
        )
    });
}

#[update]
fn set_revenue_account(new_address: Principal) {
    mutate(|state| {
        if state.revenue_account.is_none() || state.revenue_account == Some(caller()) {
            ic_cdk::println!(
                "changing the revenue account from {} to {}",
                caller(),
                new_address
            );
            state.revenue_account = Some(new_address);
        }
    })
}

// Closing of all orders is needed in order to upgrade the fees or the payment token.
// Additionally, it could help in an emergency situation.
#[update]
fn close_all_orders() {
    mutate(|state| {
        if state.revenue_account == Some(caller()) {
            state.close_orders_by_condition(&|_| true, Default::default(), 10000);
        }
    })
}

// In case something happens to the payment token, we can always switch to a new one.
#[update]
async fn set_payment_token(token_id: Principal) {
    if read(|state| state.revenue_account) != Some(caller()) {
        return;
    }

    register_token(token_id)
        .await
        .expect("couldn't register payment token");

    mutate(|state| {
        state.close_orders_by_condition(&|_| true, Default::default(), usize::MAX);
        // we need to reset the order archive because the decimals of the new payment token might
        // be different, which will lead to distorted prices
        state.order_archive.clear();
        state.payment_token_id = Some(token_id);
        state.log(format!("payment token changed to {}", token_id));
    });
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

// This method deposits liquidity from user's subaccount into the token pools.
//
// It first checks, if there's any pending liquidity on users' subaccount. If yes, it moves the
// liquidity to the corresponding token pool, and makes a corresponding accounting of the user's
// share.
//
// If the balance is smaller than the fee, the function does nothing.
#[update]
async fn deposit_liquidity(token: TokenId) -> Result<(), String> {
    let user = caller();
    let user_account = icrc1::user_account(user);
    let fee = read(|state| state.token(token))?.fee;
    let wallet_balance = icrc1::balance_of(token, &user_account)
        .await?
        // subtract fee because this funds will be moved to BEACON pool
        .checked_sub(fee)
        .unwrap_or_default();

    assert!(wallet_balance < i128::MAX as u128, "overflow");

    // if the balance is above 0, move everything from the wallet to BEACON
    if wallet_balance > 0 {
        icrc1::transfer(
            token,
            user_account.subaccount,
            icrc1::main_account(),
            wallet_balance,
            fee,
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
        );
    }
    Ok(())
}

#[update]
async fn trade(
    token: TokenId,
    amount: u128,
    price: Tokens,
    order_type: OrderType,
) -> OrderExecution {
    mutate(|state| {
        state
            .trade(order_type, caller(), token, amount, price, time())
            .expect("trade failed")
    })
}

#[update]
async fn withdraw(token: Principal) -> Result<u128, String> {
    let user = caller();
    let fee = read(|state| state.token(token))?.fee;
    let existing_balance = read(|state| state.token_pool_balance(token, user));
    assert!(existing_balance < i128::MAX as u128, "overflow");
    if existing_balance <= fee {
        return Err("amount smaller than the fee".into());
    }
    let balance = mutate_with_invarant_check(
        |state| state.withdraw_liquidity(user, token),
        Some((token, -(existing_balance as i128))),
    )?;
    let amount = balance.checked_sub(fee).expect("underflow");
    icrc1::transfer(
        token,
        None,
        Account {
            owner: user,
            subaccount: None,
        },
        amount,
        fee,
    )
    .await
    .map_err(|err| {
        let error = format!("withdraw transfer failed: {}", err);
        mutate(|state| state.log(error.clone()));
        mutate_with_invarant_check(
            |state| state.add_liquidity(user, token, balance),
            Some((token, balance as i128)),
        );
        error
    })
    .map(|_| amount)
}

#[update]
async fn list_token(token: TokenId) -> Result<(), String> {
    let user = caller();

    let Metadata { fee, decimals, .. } = read(|state| {
        state
            .token(state.payment_token_id())
            .expect("no payment token")
    });
    // we subtract the fee twice, because the user moved the funds to BEACON internal account
    // first and now we need to move it to the payment pool again
    let effective_amount = LISTING_PRICE_USD * 10_u128.pow(decimals) - fee - fee;

    if read(|state| state.payment_token_pool().get(&user) < Some(&effective_amount)) {
        return Err("not enough funds for listing".into());
    }

    // if the token listing fails, we're fine because user has the deposit added to their
    // liquidity.
    register_token(token).await?;

    // if the listing worked, charge the user
    mutate(|state| {
        state
            .charge(user, effective_amount)
            .expect("payment failed")
    });

    Ok(())
}
