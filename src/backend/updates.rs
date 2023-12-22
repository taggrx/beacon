use super::*;

#[init]
fn init() {
    kickstart();
    set_timer(Duration::from_millis(1), || {
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
        PAYMENT_TOKEN_ID
    } else {
        token
    };
    let user = caller();

    deposit_liquidity(user, pool_token).await?;

    Ok(mutate(|state| {
        state
            .trade(order_type, user, token, amount, price, ic_cdk::api::time())
            .expect("trade failed")
    }))
}

#[update]
async fn withdraw(token_id: Principal) -> Result<u128, String> {
    let user = caller();
    let fee = read(|state| state.token(token_id))?.fee;
    let existing_balance = read(|state| state.token_pool_balance(token_id, user));
    let balance = mutate_with_invarant_check(
        |state| state.withdraw_liquidity(user, token_id),
        Some((token_id, -(existing_balance as i128))),
    )?;
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

#[update]
async fn list_token(token: TokenId) -> Result<(), String> {
    let user = caller();

    deposit_liquidity(user, PAYMENT_TOKEN_ID).await?;

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
