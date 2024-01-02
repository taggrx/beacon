use ic_cdk::api::{call::arg_data_raw, canister_balance};

use super::*;

#[query]
fn orders(token: TokenId, order_type: OrderType) -> Vec<Order> {
    read(|state| state.orders(token, order_type).cloned().collect())
}

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
                .filter_map(|token_id| {
                    state
                        .order_archive
                        .get(token_id)
                        .and_then(|archive| archive.front().map(|order| (token_id, order)))
                })
                .collect::<BTreeMap<_, _>>(),
        )
    });
}

#[export_name = "canister_query executed_orders"]
fn executed_orders() {
    read(|state| {
        let token: String = parse(&arg_data_raw());
        reply(
            state
                .order_archive
                .get(&Principal::from_text(token).expect("couldn't parse principal"))
                .map(|list| list.iter().take(75).cloned().collect::<Vec<_>>())
                .unwrap_or_default(),
        )
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
    cycle_balance: u64,
    heap_size: u64,
    tokens_listed: usize,
    active_traders: usize,
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
            cycle_balance: canister_balance(),
            heap_size: heap_address().1,
            // We subtract one, because the list of tokens always contains the ICP token
            tokens_listed: state.tokens.len() - 1,
            active_traders: state.traders(),
        }
    }))
}

#[query]
fn stable_mem_read(page: u64) -> Vec<(u64, Vec<u8>)> {
    let offset = page * BACKUP_PAGE_SIZE as u64;
    let (heap_off, heap_size) = heap_address();
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
