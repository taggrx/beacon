use super::*;

#[export_name = "canister_query tokens"]
fn tokens() {
    read(|state| reply(state.tokens()));
}

#[export_name = "canister_query token_balances"]
fn token_balances() {
    reply(read(|state| state.token_balances(caller())));
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
                            .map(|archive| archive.front().map(|order| order.price()))
                            .unwrap_or_default(),
                    )
                })
                .collect::<BTreeMap<_, _>>(),
        )
    });
}

#[query]
fn orders(token: TokenId, order_type: OrderType) -> Vec<Order> {
    read(|state| state.orders(token, order_type).cloned().collect())
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

#[export_name = "canister_query logs"]
fn logs() {
    read(|state| reply(state.logs()));
}

#[derive(Serialize)]
struct BackenData {
    volume_day: u128,
    trades_day: u64,
    icp_locked: u128,
    e8s_per_xdr: u64,
    fee: u128,
}

#[export_name = "canister_query data"]
fn data() {
    let now = ic_cdk::api::time();
    reply(read(|state| {
        let day_orders = state
            .order_archive
            .values()
            .flatten()
            .filter(|order| order.executed + DAY >= now);

        BackenData {
            volume_day: day_orders.clone().map(|order| order.volume()).sum(),
            trades_day: day_orders.count() as u64,
            icp_locked: state
                .funds_under_management()
                .iter()
                .find_map(|(id, balance)| (&PAYMENT_TOKEN_ID.to_string() == id).then_some(balance))
                .copied()
                .unwrap_or_default(),
            e8s_per_xdr: state.e8s_per_xdr,
            fee: TX_FEE,
        }
    }))
}

#[query]
fn stable_mem_read(page: u64) -> Vec<(u64, Vec<u8>)> {
    let offset = page * BACKUP_PAGE_SIZE as u64;
    let (heap_off, heap_size) = memory::heap_address();
    let memory_end = heap_off + heap_size;
    if offset > memory_end {
        return Default::default();
    }
    let chunk_size = (BACKUP_PAGE_SIZE as u64).min(memory_end - offset) as usize;
    let mut buf = Vec::with_capacity(chunk_size);
    buf.spare_capacity_mut();
    unsafe {
        buf.set_len(chunk_size);
    }
    ic_cdk::api::stable::stable64_read(offset, &mut buf);
    vec![(page, buf)]
}
