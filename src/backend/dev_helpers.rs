use super::*;
use ic_cdk::api;

#[update]
fn replace_canister_id(old: Principal, new: Principal) {
    unsafe_mutate(|state| state.replace_canister_id(old, new))
}

#[update]
fn replace_user_id(old: Principal, new: Principal) {
    unsafe_mutate(|state| state.replace_user_id(old, new))
}

#[update]
fn stable_mem_write(input: Vec<(u64, Vec<u8>)>) {
    if let Some((page, buffer)) = input.get(0) {
        if buffer.is_empty() {
            return;
        }
        let offset = page * BACKUP_PAGE_SIZE as u64;
        let current_size = api::stable::stable64_size();
        let needed_size = ((offset + buffer.len() as u64) >> 16) + 1;
        let delta = needed_size.saturating_sub(current_size);
        if delta > 0 {
            api::stable::stable64_grow(delta).unwrap_or_else(|_| panic!("couldn't grow memory"));
        }
        api::stable::stable64_write(offset, buffer);
    }
}

#[update]
fn stable_to_heap() {
    stable_to_heap_core();
}
