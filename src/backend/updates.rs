use crate::order_book::OrderExecution;

use super::*;

#[init]
fn init() {
    kickstart();
    // register ICP as payment tokens
    set_timer(Duration::from_millis(0), || {
        spawn(async {
            register_token(PAYMENT_TOKEN_ID)
                .await
                .expect("couldn't register payment token");
            mutate(|state| {
                state.tokens.get_mut(&PAYMENT_TOKEN_ID).expect("no payment token").logo = Some("data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAACQAAAAkCAMAAADW3miqAAAAqFBMVEUAAAAPBQYCDBDpHnj8sDruHXknpNopq+IEAgLyWiQnmsvsWCMQQVY5HQ1ZJYQcdZrbHXW6H3oIIzCxeSplLRCnFlnvnjRrJIAkgrG2Tx33hy+YIXskCB0efqdMHF55Dz70bShSnsQVNkaFJIASSmF8LhJkDDRKQ5kyZH6ePhjfUyHgeCt4GFs9Ej9MMXzLqlmBH1U/CiazbFayQ5GLYSG7r258STiDf1U4y9K8AAABVUlEQVQ4y+2SyZaCMBREEwgQw6jMowqKOE9t9///WecRBOzeurSWde6pNyL00XslGTnT1MiQBgt7sa7vYg8/jVmuaiDVnnUONWN9wrVYxKZwSqY9xQzBeLuJYBbJ3mtzWLTZnE5NBHHqCpg1xOy223ifJMmeZ0nNxrKKjPLGIkG1jL41KVRNksDFqARGtDezgTI8YNbCwm5AAg8VlhVmzymBYjfOeLSzzIAQF52ssKBoTC1vk3XvUJeQAFlheBxtLAfqGw/OFyGEQ9UIwpclpyLpDxSG1VCOzuU7ULb0Wq6oqjQbGEW5szElGj9WaXro5gVGrg3YanchzIPOJcKHNJ0eMticD8ycopXaXkjiFmceDU/N0imXf/EdRTBIUFrU/JzPjyVrM6+ccRxZ4TlOTf/dvBTNZT4wPMa/9t9jd9+T99+Da9+RHb/GL3+oqiwf/+FHb9EvACscm39NBowAAAAASUVORK5CYII=".into())
            })
        })
    });
}

#[pre_upgrade]
fn pre_upgrade() {
    mutate(heap_to_stable)
}

#[post_upgrade]
fn post_upgrade() {
    stable_to_heap_core();
    kickstart();
}

#[update]
fn set_revenue_account(new_address: Principal) {
    mutate(|state| {
        if state.revenue_account.is_none() || state.revenue_account == Some(caller()) {
            state.revenue_account = Some(new_address);
        }
    })
}

// Closing of all orders is is needed in order to upgrading the fees or payment token.
// Additionally, it could help in an emergency situation.
#[update]
fn close_all_orders() {
    mutate(|state| {
        if state.revenue_account == Some(caller()) {
            state.close_orders_by_condition(&|_| true, Default::default(), 10000);
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
// This method deposits liquidity from user's subaccount into the token pools.
//
// It first checks, if there's any pending liquidity on users' subaccount. If yes, it moves the
// liquidity to the corresponding token pool, and makes a corresponding accounting of the user's
// share.
//
// If the balance is smaller than the fee, the function does nothing.
async fn deposit_liquidity(token: TokenId) -> Result<(), String> {
    let user = caller();
    let user_account = icrc1::user_account(user);
    let fee = read(|state| state.token(token))?.fee;
    let wallet_balance = icrc1::balance_of(token, &user_account)
        .await?
        // subtract fee becasue this funds will be moved to BEACON pool
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
            .trade(
                order_type,
                caller(),
                token,
                amount,
                price,
                ic_cdk::api::time(),
            )
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
    let listing_price = read(|state| state.e8s_per_xdr * 100);

    // we subtract the fees twice, because the user moved the funds to BEACON internal account
    // first and now we need to move it to the payment pool again
    let effective_amount = (ICP::from_e8s(listing_price) - DEFAULT_FEE - DEFAULT_FEE).e8s() as u128;

    if read(|state| state.payment_token_pool().get(&user) < Some(&effective_amount)) {
        return Err("not enough funds for listing".into());
    }

    // if the token listing fails, we're fine becasue user has the deposit added to their
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

async fn register_token(token: TokenId) -> Result<(), String> {
    let metadata = icrc1::metadata(token)
        .await
        .map_err(|err| format!("couldn't fetch metadata: {}", err))?;
    mutate_with_invarant_check(|state| state.list_token(token, metadata), Some((token, 0)))
}
