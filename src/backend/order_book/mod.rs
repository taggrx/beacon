use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
};

use candid::Principal;
use serde::{Deserialize, Serialize};

pub type Tokens = u64;
pub type TokenId = Principal;
pub type E8sPerToken = u64;
type E8s = u64;

const TX_FEE: u64 = 15; // 0.15% per trade side

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
struct Order {
    owner: Principal,
    executor: Option<Principal>,
    amount: Tokens,
    price: E8sPerToken,
    executed: Timestamp,
}

impl PartialOrd for Order {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.price.cmp(&other.price))
    }
}

impl Ord for Order {
    fn cmp(&self, other: &Self) -> Ordering {
        self.price.cmp(&other.price)
    }
}

#[derive(Serialize, Deserialize)]
struct Book {
    buyers: BTreeSet<Order>,
    sellers: BTreeSet<Order>,
}

type Timestamp = u64;

#[derive(Clone, Serialize, Deserialize)]
pub struct Metadata {
    symbol: String,
    fee: Tokens,
    decimals: u32,
    logo: Option<String>,
}

#[derive(Default, Serialize, Deserialize)]
pub struct State {
    orders: BTreeMap<TokenId, Book>,
    order_archive: BTreeMap<TokenId, Vec<Order>>,
    pools: BTreeMap<TokenId, BTreeMap<Principal, Tokens>>,
    icp_pool: BTreeMap<Principal, E8s>,
    tokens: BTreeMap<TokenId, Metadata>,
    pub e8s_per_xdr: u64,
    pub revenue_account: Option<Principal>,
}

impl State {
    pub fn get_token(&self, id: TokenId) -> Result<Metadata, String> {
        self.tokens
            .get(&id)
            .cloned()
            .ok_or("no token listed".into())
    }

    pub fn add_token(
        &mut self,
        id: TokenId,
        symbol: String,
        fee: Tokens,
        decimals: u32,
        logo: Option<String>,
    ) {
        self.tokens.insert(
            id,
            Metadata {
                symbol,
                logo,
                fee,
                decimals,
            },
        );
    }

    pub async fn buy(
        &mut self,
        buyer: Principal,
        token: TokenId,
        mut amount: E8s,
        limit: Option<E8sPerToken>,
        time: Timestamp,
    ) -> Result<(), String> {
        if amount < *self.icp_pool.get(&buyer).unwrap_or(&0) {
            return Err("not enough ICP funds".into());
        }

        let sell_orders = &mut self
            .orders
            .get_mut(&token)
            .ok_or("no orders found")?
            .sellers;

        let archive = self.order_archive.entry(token).or_default();

        while let Some(mut order) = sell_orders.pop_first() {
            // check the limit
            if let Some(limit) = limit {
                if limit > order.price {
                    sell_orders.insert(order);
                    break;
                }
            }

            let e8s = order.amount * order.price;
            amount = match amount.checked_sub(e8s) {
                // if we detect an order larger than buyer's funds, stop
                None => {
                    sell_orders.insert(order);
                    break;
                }
                Some(new_amount) => new_amount,
            };

            // adjust token pool amounts
            let token_pool = self.pools.get_mut(&token).expect("no token pool found");
            let sellers_tokens = token_pool.get_mut(&order.owner).expect("no tokens in pool");
            *sellers_tokens = sellers_tokens
                .checked_sub(order.amount)
                .expect("not enough tokens");
            let buyers_tokens = token_pool.entry(buyer).or_insert(0);
            *buyers_tokens += order.amount;

            let fee = e8s * 15 / 10000;

            // adjust icp pool amounts
            let buyers_icp_tokens = self.icp_pool.get_mut(&buyer).expect("no ICP tokens");
            *buyers_icp_tokens = buyers_icp_tokens
                .checked_sub(e8s + fee)
                .expect("not enough ICP tokens");
            let sellers_icp_tokens = self.icp_pool.entry(buyer).or_default();
            *sellers_icp_tokens += e8s.checked_sub(fee).expect("amount smaller than fees");
            let icp_fees = self
                .icp_pool
                .entry(self.revenue_account.expect("no revenue account set"))
                .or_default();
            *icp_fees += 2 * fee;

            order.executor = Some(buyer);
            order.executed = time;
            archive.push(order);
        }

        Ok(())
    }
}
