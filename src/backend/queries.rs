use super::*;

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
