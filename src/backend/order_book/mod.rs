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
    amount: Tokens,
    price: E8sPerToken,
    executor: Option<Principal>,
    executed: Option<Timestamp>,
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

#[derive(Default, Serialize, Deserialize)]
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

    pub fn create_order(
        &mut self,
        caller: Principal,
        token: TokenId,
        amount: Tokens,
        price: E8sPerToken,
        order_type: &str,
    ) -> Result<(), String> {
        if !self.tokens.contains_key(&token) {
            return Err("token not listed".into());
        }

        if amount
            > self
                .pools
                .get(&token)
                .ok_or("no token found")?
                .get(&caller)
                .copied()
                .unwrap_or_default()
        {
            return Err("funds not available".into());
        }

        let order = Order {
            owner: caller,
            amount,
            price,
            executor: None,
            executed: None,
        };
        let order_book = self.orders.entry(token).or_default();
        if order_type == "buy" {
            order_book.buyers.insert(order);
        } else {
            order_book.sellers.insert(order);
        }
        Ok(())
    }

    pub fn sell(
        &mut self,
        seller: Principal,
        token: TokenId,
        mut amount: Tokens,
        limit: Option<E8sPerToken>,
        time: Timestamp,
    ) -> Result<(), String> {
        if amount
            > *self
                .pools
                .get(&token)
                .ok_or("no token pool")?
                .get(&seller)
                .unwrap_or(&0)
        {
            return Err("not enough tokens".into());
        }

        let buy_orders = &mut self
            .orders
            .get_mut(&token)
            .ok_or("no buy orders found")?
            .buyers;

        let archive = self.order_archive.entry(token).or_default();

        while let Some(mut order) = buy_orders.pop_first() {
            if limit > Some(order.price) || amount < order.amount {
                buy_orders.insert(order);
                break;
            }

            amount = amount
                .checked_sub(order.amount)
                .ok_or("available funds are too small")?;

            trade(
                &mut self.pools,
                &mut self.icp_pool,
                seller,
                order.owner,
                token,
                order.amount,
                order.amount * order.price,
                self.revenue_account.unwrap(),
            )?;

            order.executor = Some(seller);
            order.executed = Some(time);
            archive.push(order);
        }

        Ok(())
    }

    pub fn buy(
        &mut self,
        buyer: Principal,
        token: TokenId,
        mut amount: E8s,
        limit: Option<E8sPerToken>,
        time: Timestamp,
    ) -> Result<(), String> {
        let max_fee = amount * 15 / 10000;
        if amount + max_fee < *self.icp_pool.get(&buyer).unwrap_or(&0) {
            return Err("not enough ICP funds".into());
        }

        let sell_orders = &mut self
            .orders
            .get_mut(&token)
            .ok_or("no sell orders found")?
            .sellers;

        let archive = self.order_archive.entry(token).or_default();

        while let Some(mut order) = sell_orders.pop_first() {
            let volume = order.amount * order.price;
            if limit > Some(order.price) || volume > amount {
                sell_orders.insert(order);
                break;
            }

            amount = amount
                .checked_sub(volume)
                .ok_or("available funds are too small")?;

            trade(
                &mut self.pools,
                &mut self.icp_pool,
                order.owner,
                buyer,
                token,
                order.amount,
                volume,
                self.revenue_account.unwrap(),
            )?;

            order.executor = Some(buyer);
            order.executed = Some(time);
            archive.push(order);
        }

        Ok(())
    }
}

fn trade(
    pools: &mut BTreeMap<TokenId, BTreeMap<Principal, Tokens>>,
    icp_pool: &mut BTreeMap<Principal, E8s>,
    seller: Principal,
    buyer: Principal,
    token: TokenId,
    amount: Tokens,
    volume: E8s,
    revenue_account: Principal,
) -> Result<(), String> {
    // adjust token pool amounts
    let token_pool = pools.get_mut(&token).ok_or("no token pool found")?;
    let sellers_tokens = token_pool.get_mut(&seller).ok_or("no tokens in pool")?;
    *sellers_tokens = sellers_tokens
        .checked_sub(amount)
        .ok_or("not enough tokens")?;
    let buyers_tokens = token_pool.entry(buyer).or_insert(0);
    *buyers_tokens += amount;

    let fee = volume * TX_FEE / 10000;

    // adjust icp pool amounts
    let buyers_icp_tokens = icp_pool.get_mut(&buyer).ok_or("no ICP tokens")?;
    *buyers_icp_tokens = buyers_icp_tokens
        .checked_sub(volume + fee)
        .ok_or("not enough ICP tokens")?;
    let sellers_icp_tokens = icp_pool.entry(buyer).or_default();
    *sellers_icp_tokens += volume.checked_sub(fee).ok_or("amount smaller than fees")?;
    let icp_fees = icp_pool.entry(revenue_account).or_default();
    *icp_fees += 2 * fee;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn pr(n: u8) -> Principal {
        let v = vec![0, n];
        Principal::from_slice(&v)
    }

    #[test]
    fn test_selling() {
        let mut state = State::default();

        let taggr_token = pr(100);

        assert_eq!(
            state.create_order(pr(0), taggr_token, 7, 50000000, "buy"),
            Err("token not listed".into())
        );

        state.add_token(taggr_token, "TAGGR".into(), 25, 2, None);

        // buy order for 7 $TAGGR / 0.5 ICP each
        assert!(state
            .create_order(pr(0), taggr_token, 7, 50000000, "buy")
            .is_ok());
        // buy order for 16 $TAGGR / 0.3 ICP each
        assert!(state
            .create_order(pr(0), taggr_token, 16, 30000000, "buy")
            .is_ok());
        // buy order for 25 $TAGGR / 0.1 ICP each
        assert!(state
            .create_order(pr(0), taggr_token, 25, 10000000, "buy")
            .is_ok());
    }
}
