use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
};

use candid::Principal;
use ic_ledger_types::{DEFAULT_FEE, MAINNET_LEDGER_CANISTER_ID};
use serde::{Deserialize, Serialize};

pub type Tokens = u64;
pub type TokenId = Principal;
pub type E8sPerToken = u64;
pub type E8s = u64;

const TX_FEE: u64 = 15; // 0.15% per trade side

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
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
    pub symbol: String,
    pub fee: Tokens,
    pub decimals: u32,
    pub logo: Option<String>,
}

#[derive(Default, Serialize, Deserialize)]
pub struct State {
    orders: BTreeMap<TokenId, Book>,
    order_archive: BTreeMap<TokenId, Vec<Order>>,
    pools: BTreeMap<TokenId, BTreeMap<Principal, Tokens>>,
    tokens: BTreeMap<TokenId, Metadata>,
    pub e8s_per_xdr: u64,
    pub revenue_account: Option<Principal>,
}

impl State {
    pub fn token_balances(&self, principal: Principal) -> BTreeMap<TokenId, Tokens> {
        self.pools
            .iter()
            .filter_map(|(id, pool)| pool.get(&principal).map(|balance| (*id, *balance)))
            .collect()
    }

    pub fn tokens(&self) -> &'_ BTreeMap<TokenId, Metadata> {
        &self.tokens
    }

    pub fn token(&self, id: TokenId) -> Result<Metadata, String> {
        self.tokens
            .get(&id)
            .cloned()
            .ok_or("no token listed".into())
    }

    pub fn add_liquidity(
        &mut self,
        caller: Principal,
        id: TokenId,
        amount: Tokens,
    ) -> Result<(), String> {
        self.pools
            .get_mut(&id)
            .ok_or("token not found")?
            .entry(caller)
            .and_modify(|funds| {
                *funds += amount;
            })
            .or_insert(amount);
        Ok(())
    }

    pub fn withdraw_liquidity(&mut self, caller: Principal, id: TokenId) -> Result<Tokens, String> {
        let fee = self.tokens.get(&id).ok_or("no token found")?.fee;
        let pool = self.pools.get_mut(&id).ok_or("no token found")?;
        if fee > pool.get(&caller).copied().unwrap_or_default() {
            return Err("amount smaller than transaction fee".into());
        }
        pool.remove(&caller)
            .map(|tokens| tokens.saturating_sub(fee))
            .ok_or("nothing to withdraw".into())
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
        self.pools.insert(id, Default::default());
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

        let order = Order {
            owner: caller,
            amount,
            price,
            executor: None,
            executed: None,
        };
        let buying = order_type == "buy";
        let order_book = self.orders.entry(token).or_default();
        let token_balance = self
            .pools
            .get(&if buying {
                MAINNET_LEDGER_CANISTER_ID
            } else {
                token
            })
            .ok_or("no token found")?
            .get(&caller)
            .copied()
            .unwrap_or_default();
        if buying {
            let volume = amount * price;
            let max_fee = trading_fee(volume);
            if volume + max_fee > token_balance {
                return Err("not enough funds available for this order size".into());
            }
            order_book.buyers.insert(order);
        } else {
            if amount > token_balance {
                return Err("not enough tokens available for this order size".into());
            }

            order_book.sellers.insert(order);
        }
        Ok(())
    }

    pub fn trade(
        &mut self,
        trade_type: &str,
        trader: Principal,
        token: TokenId,
        mut amount: u64,
        limit: Option<E8sPerToken>,
        time: Timestamp,
    ) -> Result<(), String> {
        let icp_pool_tokens: E8s = self
            .pools
            .get(&MAINNET_LEDGER_CANISTER_ID)
            .ok_or("no icp pool")?
            .values()
            .sum();
        let pool_tokens: Tokens = self
            .pools
            .get(&token)
            .ok_or("no pool found")?
            .values()
            .sum();

        let buying = trade_type == "buy";

        if !buying
            && amount
                > *self
                    .pools
                    .get(&token)
                    .ok_or("no token pool")?
                    .get(&trader)
                    .unwrap_or(&0)
        {
            return Err("not enough tokens".into());
        }

        let book = &mut self.orders.get_mut(&token).ok_or("no orders found")?;

        let orders = if buying {
            &mut book.sellers
        } else {
            &mut book.buyers
        };

        let archive = self.order_archive.entry(token).or_default();

        while let Some(mut order) = if buying {
            orders.pop_first()
        } else {
            orders.pop_last()
        } {
            // limit checks
            if buying && limit.is_some() && limit < Some(order.price)
                || !buying && limit > Some(order.price)
            {
                orders.insert(order);
                break;
            }

            amount = if order.amount > amount {
                // partial order fill - create a new one for left overs
                let volume = order.price * amount;
                if buying
                    && volume + trading_fee(volume)
                        > *self
                            .pools
                            .get(&MAINNET_LEDGER_CANISTER_ID)
                            .ok_or("icp pool not found")?
                            .get(&trader)
                            .unwrap_or(&0)
                {
                    orders.insert(order);
                    break;
                }
                let mut remaining_order = order.clone();
                remaining_order.amount = order.amount - amount;
                orders.insert(remaining_order);
                order.amount = amount;
                0
            } else {
                amount - order.amount
            };

            let (seller, buyer) = if buying {
                (order.owner, trader)
            } else {
                (trader, order.owner)
            };

            adjust_pools(
                &mut self.pools,
                seller,
                buyer,
                token,
                order.amount,
                order.amount * order.price,
                self.revenue_account.unwrap(),
            )?;

            order.executor = Some(trader);
            order.executed = Some(time);
            archive.push(order);

            if amount == 0 {
                break;
            }
        }

        if icp_pool_tokens
            != self
                .pools
                .get(&MAINNET_LEDGER_CANISTER_ID)
                .ok_or("no icp pool found")?
                .values()
                .sum::<E8s>()
        {
            return Err("icp pool invariant violated".into());
        }
        if pool_tokens
            != self
                .pools
                .get(&token)
                .ok_or("no pool found")?
                .values()
                .sum::<Tokens>()
        {
            return Err("token pool invariant violated".into());
        };

        Ok(())
    }
}

fn adjust_pools(
    pools: &mut BTreeMap<TokenId, BTreeMap<Principal, Tokens>>,
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

    let fee = trading_fee(volume);

    // adjust icp pool amounts
    let icp_pool = pools
        .get_mut(&MAINNET_LEDGER_CANISTER_ID)
        .ok_or("no icp pool found")?;
    let buyers_icp_tokens = icp_pool.get_mut(&buyer).ok_or("no ICP tokens")?;
    *buyers_icp_tokens = buyers_icp_tokens
        .checked_sub(volume + fee)
        .ok_or("not enough ICP tokens")?;
    let sellers_icp_tokens = icp_pool.entry(seller).or_default();
    *sellers_icp_tokens += volume.checked_sub(fee).ok_or("amount smaller than fees")?;
    let icp_fees = icp_pool.entry(revenue_account).or_default();
    *icp_fees += 2 * fee;

    Ok(())
}

fn trading_fee(volume: E8s) -> E8s {
    volume * TX_FEE / 10000
}

#[cfg(test)]
mod tests {

    use super::*;

    pub fn pr(n: u8) -> Principal {
        let v = vec![0, n];
        Principal::from_slice(&v)
    }

    fn icp_pool(state: &State) -> &'_ BTreeMap<Principal, Tokens> {
        state.pools.get(&MAINNET_LEDGER_CANISTER_ID).unwrap()
    }

    #[test]
    fn test_liquidity_adding_and_withdrawals() {
        let mut state = State::default();
        state
            .pools
            .insert(MAINNET_LEDGER_CANISTER_ID, Default::default());
        state.tokens.insert(
            MAINNET_LEDGER_CANISTER_ID,
            Metadata {
                symbol: "ICP".into(),
                fee: DEFAULT_FEE.e8s(),
                decimals: 8,
                logo: None,
            },
        );

        let token = pr(100);

        assert_eq!(
            state.add_liquidity(pr(0), token, 111),
            Err("token not found".into())
        );
        state.add_token(
            token,
            "TAGGR".into(),
            25, // fees
            2,  // decimals
            None,
        );

        state.add_liquidity(pr(0), token, 111).unwrap();
        state.add_liquidity(pr(0), token, 222).unwrap();
        assert_eq!(
            state.withdraw_liquidity(pr(1), token),
            Err("amount smaller than transaction fee".into())
        );
        assert_eq!(state.withdraw_liquidity(pr(0), token), Ok(333 - 25));

        let one_icp = 100000000;
        state
            .add_liquidity(pr(0), MAINNET_LEDGER_CANISTER_ID, one_icp)
            .unwrap();
        assert_eq!(
            state.withdraw_liquidity(pr(1), MAINNET_LEDGER_CANISTER_ID),
            Err("amount smaller than transaction fee".into())
        );
        assert_eq!(
            state.withdraw_liquidity(pr(0), MAINNET_LEDGER_CANISTER_ID),
            Ok((ic_ledger_types::Tokens::from_e8s(one_icp) - DEFAULT_FEE).e8s())
        );
    }

    #[test]
    fn test_selling() {
        let mut state = State::default();
        state
            .pools
            .insert(MAINNET_LEDGER_CANISTER_ID, Default::default());
        state.tokens.insert(
            MAINNET_LEDGER_CANISTER_ID,
            Metadata {
                symbol: "ICP".into(),
                fee: DEFAULT_FEE.e8s(),
                decimals: 8,
                logo: None,
            },
        );

        state.revenue_account = Some(pr(255));

        let token = pr(100);

        assert_eq!(
            state.create_order(pr(0), token, 7, 50000000, "buy"),
            Err("token not listed".into())
        );

        state.add_token(token, "TAGGR".into(), 25, 2, None);

        // buy order for 7 $TAGGR / 0.1 ICP each
        assert_eq!(
            state.create_order(pr(0), token, 7, 10000000, "buy"),
            Err("not enough funds available for this order size".into())
        );

        state
            .add_liquidity(pr(0), MAINNET_LEDGER_CANISTER_ID, 8 * 10000000)
            .unwrap();
        assert!(state.create_order(pr(0), token, 7, 10000000, "buy").is_ok());

        // buy order for 16 $TAGGR / 0.03 ICP each
        state
            .add_liquidity(pr(1), MAINNET_LEDGER_CANISTER_ID, 17 * 30000000)
            .unwrap();
        assert!(state.create_order(pr(1), token, 16, 3000000, "buy").is_ok());

        // buy order for 25 $TAGGR / 0.01 ICP each
        state
            .add_liquidity(pr(2), MAINNET_LEDGER_CANISTER_ID, 24 * 1000000)
            .unwrap();
        assert_eq!(
            state.create_order(pr(2), token, 25, 1000000, "buy"),
            Err("not enough funds available for this order size".into())
        );
        state
            .add_liquidity(pr(2), MAINNET_LEDGER_CANISTER_ID, 2 * 1000000)
            .unwrap();
        assert!(state.create_order(pr(2), token, 25, 1000000, "buy").is_ok());

        // buyer has 26 * 0.01 ICP
        assert_eq!(icp_pool(&state).get(&pr(2)).unwrap(), &(26 * 1000000));

        let buyer_orders = &state.orders.get(&token).unwrap().buyers;
        assert_eq!(
            buyer_orders
                .iter()
                .map(|order| order.price)
                .collect::<Vec<_>>(),
            vec![1000000, 3000000, 10000000]
        );
        let best_order = buyer_orders.last().unwrap();
        assert_eq!(best_order.amount, 7);
        assert_eq!(best_order.price, 10000000);

        // three buyers
        assert_eq!(icp_pool(&state).len(), 3);

        let seller = pr(5);

        assert_eq!(
            state.trade("sell", seller, token, 5, None, 123456),
            Err("not enough tokens".into())
        );
        state.add_liquidity(seller, token, 250).unwrap();
        assert_eq!(state.trade("sell", seller, token, 5, None, 123456), Ok(()));

        // verify the partial filling
        let buyer_orders = &state.orders.get(&token).unwrap().buyers;
        // we still have 3 orders
        assert_eq!(buyer_orders.len(), 3);
        let best_order = buyer_orders.last().unwrap();
        // less tokens to buy at the given price as before
        assert_eq!(best_order.amount, 2);
        assert_eq!(best_order.price, 10000000);

        let archived_orders = state.order_archive.get(&token).unwrap();
        assert_eq!(archived_orders.len(), 1);
        let executed_order = archived_orders.first().unwrap();
        assert_eq!(executed_order.executed, Some(123456));
        assert_eq!(executed_order.executor, Some(seller));
        // only 5 tokens got traded
        assert_eq!(executed_order.amount, 5);

        // buyer got 5 tokens
        assert_eq!(state.pools.get(&token).unwrap().get(&pr(0)).unwrap(), &5);

        // now seller should get a balance too, plus the fee acount
        assert_eq!(icp_pool(&state).len(), 5);
        // seller has expected amount of ICP: 5 * 0.1 ICP - fee
        let volume = 50000000;
        let fee_per_side = trading_fee(volume);
        assert_eq!(
            icp_pool(&state).get(&seller).unwrap(),
            &(volume - fee_per_side)
        );
        // buyer should have previous amount - volume - fee;
        assert_eq!(
            icp_pool(&state).get(&pr(0)).unwrap(),
            &(8 * 10000000 - volume - fee_per_side)
        );
        // fee acount has 2 fees
        assert_eq!(icp_pool(&state).get(&pr(255)).unwrap(), &(2 * fee_per_side));

        // let's sell more
        // at that point we have buy orders: 25 @ 0.01, 16 @ 0.03, 2 @ 0.1
        assert_eq!(state.trade("sell", seller, token, 10, None, 123457), Ok(()));

        // we should have only two now
        let buyer_orders = &state.orders.get(&token).unwrap().buyers;
        assert_eq!(buyer_orders.len(), 2);
        let best_order = buyer_orders.last().unwrap();
        assert_eq!(best_order.amount, 8);
        assert_eq!(best_order.price, 3000000);

        // seller still has 250 - 15 tokens
        assert_eq!(
            state.pools.get(&token).unwrap().get(&seller).unwrap(),
            &(250 - 15)
        );

        // at that point we have buy orders: 11 @ 0.03, 7 @ 0.05
        assert_eq!(
            state.trade("sell", seller, token, 150, None, 123457),
            Ok(())
        );

        // seller still has 250 - 30 - 18 tokens
        assert_eq!(
            state.pools.get(&token).unwrap().get(&seller).unwrap(),
            &(250 - 30 - 18)
        );
        // all buyer got their tokens
        assert_eq!(state.pools.get(&token).unwrap().get(&pr(0)).unwrap(), &7);
        assert_eq!(state.pools.get(&token).unwrap().get(&pr(1)).unwrap(), &16);
        assert_eq!(state.pools.get(&token).unwrap().get(&pr(2)).unwrap(), &25);

        // executed orders: 25 @ 0.1, 16 @ 0.03, 7 @ 0.05
        let (v1, v2, v3) = (25 * 1000000, 16 * 3000000, 7 * 10000000);
        let fees = trading_fee(v1) + trading_fee(v2) + trading_fee(v3);
        assert_eq!(
            icp_pool(&state).get(&seller).unwrap(),
            &(v1 + v2 + v3 - fees)
        );
        assert_eq!(icp_pool(&state).get(&pr(255)).unwrap(), &(2 * fees));
    }

    #[test]
    fn test_buying() {
        let mut state = State::default();
        state
            .pools
            .insert(MAINNET_LEDGER_CANISTER_ID, Default::default());
        state.tokens.insert(
            MAINNET_LEDGER_CANISTER_ID,
            Metadata {
                symbol: "ICP".into(),
                fee: DEFAULT_FEE.e8s(),
                decimals: 8,
                logo: None,
            },
        );

        state.revenue_account = Some(pr(255));

        let token = pr(100);

        assert_eq!(
            state.create_order(pr(0), token, 7, 5000000, "sell"),
            Err("token not listed".into())
        );

        state.add_token(token, "TAGGR".into(), 25, 2, None);

        // sell order for 7 $TAGGR / 0.05 ICP each
        assert_eq!(
            state.create_order(pr(0), token, 7, 5000000, "sell"),
            Err("not enough tokens available for this order size".into())
        );

        state.add_liquidity(pr(0), token, 7).unwrap();
        assert!(state.create_order(pr(0), token, 7, 5000000, "sell").is_ok());

        // sell order for 16 $TAGGR / 0.03 ICP each
        state.add_liquidity(pr(1), token, 16).unwrap();
        assert!(state
            .create_order(pr(1), token, 16, 3000000, "sell")
            .is_ok());

        // sell order for 25 $TAGGR / 1 ICP each
        state.add_liquidity(pr(2), token, 24).unwrap();
        assert_eq!(
            state.create_order(pr(2), token, 25, 100000000, "sell"),
            Err("not enough tokens available for this order size".into())
        );
        state.add_liquidity(pr(2), token, 1).unwrap();
        assert!(state
            .create_order(pr(2), token, 25, 100000000, "sell")
            .is_ok());

        // Order book: 16 @ 0.03, 7 @ 0.05, 25 @ 1
        let sell_orders = &state.orders.get(&token).unwrap().sellers;
        assert_eq!(sell_orders.len(), 3);
        let best_order = sell_orders.first().unwrap();
        assert_eq!(best_order.amount, 16);
        assert_eq!(best_order.price, 3000000);

        // three sellers
        assert_eq!(state.pools.get(&token).unwrap().len(), 3);

        let buyer = pr(5);

        assert_eq!(state.trade("buy", buyer, token, 10, None, 123456), Ok(()));
        // since we had no ICP we didn't buy anything
        assert_eq!(state.pools.get(&token).unwrap().get(&buyer), None);
        state
            .add_liquidity(buyer, MAINNET_LEDGER_CANISTER_ID, 12 * 3000000)
            .unwrap();
        assert_eq!(icp_pool(&state).len(), 1);
        assert_eq!(state.trade("buy", buyer, token, 10, None, 123456), Ok(()));

        // verify the partial filling
        let sell_orders = &state.orders.get(&token).unwrap().sellers;
        // we still have 3 orders
        assert_eq!(sell_orders.len(), 3);
        let best_order = sell_orders.first().unwrap();
        // less tokens to buy at the given price as before
        assert_eq!(best_order.amount, 6);
        assert_eq!(best_order.price, 3000000);

        let archived_orders = state.order_archive.get(&token).unwrap();
        assert_eq!(archived_orders.len(), 1);
        let executed_order = archived_orders.first().unwrap();
        assert_eq!(executed_order.executed, Some(123456));
        assert_eq!(executed_order.executor, Some(buyer));
        // only 5 tokens got traded
        assert_eq!(executed_order.amount, 10);

        // buyer got 10 tokens
        assert_eq!(state.pools.get(&token).unwrap().get(&buyer).unwrap(), &10);

        // now seller should get a balance too, plus the fee acount
        assert_eq!(icp_pool(&state).len(), 3);

        // let's buy more
        // at that point we have buy orders: 6 @ 0.03, 7 @ 0.05, 25 @ 1
        state
            .add_liquidity(buyer, MAINNET_LEDGER_CANISTER_ID, 6 * 3000000 + 2 * 5000000)
            .unwrap();
        assert_eq!(state.trade("buy", buyer, token, 7, None, 123457), Ok(()));
        // buyer got 17 tokens
        assert_eq!(state.pools.get(&token).unwrap().get(&buyer).unwrap(), &17);

        // we should have only two now
        let sell_orders = &state.orders.get(&token).unwrap().sellers;
        assert_eq!(sell_orders.len(), 2);
        let best_order = sell_orders.first().unwrap();
        assert_eq!(best_order.amount, 6);
        assert_eq!(best_order.price, 5000000);

        state
            .add_liquidity(
                buyer,
                MAINNET_LEDGER_CANISTER_ID,
                6 * 5000000 + 28 * 100000000,
            )
            .unwrap();

        assert_eq!(state.trade("buy", buyer, token, 100, None, 123458), Ok(()));

        // all sellers got ICP
        let (v2, v1, v3) = (16 * 3000000, 7 * 5000000, 25 * 100000000);
        assert_eq!(
            icp_pool(&state).get(&pr(0)).unwrap(),
            &(v1 - trading_fee(v1))
        );
        assert_eq!(
            icp_pool(&state).get(&pr(1)).unwrap(),
            &(v2 - trading_fee(v2))
        );
        assert_eq!(
            icp_pool(&state).get(&pr(2)).unwrap(),
            &(v3 - trading_fee(v3))
        );

        // executed orders: 16 @ 0.03, 7 @ 0.05, 25 @ 1
        let fees = trading_fee(v1) + trading_fee(v2) + trading_fee(v3);
        assert_eq!(icp_pool(&state).get(&pr(255)).unwrap(), &(2 * fees));
    }

    #[test]
    fn test_limit_selling() {
        let mut state = State::default();
        state
            .pools
            .insert(MAINNET_LEDGER_CANISTER_ID, Default::default());
        state.tokens.insert(
            MAINNET_LEDGER_CANISTER_ID,
            Metadata {
                symbol: "ICP".into(),
                fee: DEFAULT_FEE.e8s(),
                decimals: 8,
                logo: None,
            },
        );

        state.revenue_account = Some(pr(255));

        let token = pr(100);

        state.add_token(token, "TAGGR".into(), 25, 2, None);

        // buy order for 7 $TAGGR / 0.1 ICP each
        state
            .add_liquidity(pr(0), MAINNET_LEDGER_CANISTER_ID, 8 * 10000000)
            .unwrap();
        assert!(state.create_order(pr(0), token, 7, 10000000, "buy").is_ok());

        // buy order for 16 $TAGGR / 0.03 ICP each
        state
            .add_liquidity(pr(1), MAINNET_LEDGER_CANISTER_ID, 17 * 30000000)
            .unwrap();
        assert!(state.create_order(pr(1), token, 16, 3000000, "buy").is_ok());

        // buy order for 25 $TAGGR / 0.01 ICP each
        state
            .add_liquidity(pr(2), MAINNET_LEDGER_CANISTER_ID, 26 * 1000000)
            .unwrap();
        assert!(state.create_order(pr(2), token, 25, 1000000, "buy").is_ok());

        // Orer book: 7 @ 0.1, 16 @ 0.03, 25 @ 0.01

        let seller = pr(5);

        state.add_liquidity(seller, token, 250).unwrap();
        assert_eq!(
            state.trade("sell", seller, token, 50, Some(2000000), 123456),
            Ok(())
        );

        // 2 orders were filled
        let buyer_orders = &state.orders.get(&token).unwrap().buyers;
        assert_eq!(buyer_orders.len(), 1);
        let best_order = buyer_orders.last().unwrap();
        // order below the limit wasn't touched
        assert_eq!(best_order.amount, 25);

        // only two buyer got their tokens
        assert_eq!(state.pools.get(&token).unwrap().get(&pr(0)).unwrap(), &7);
        assert_eq!(state.pools.get(&token).unwrap().get(&pr(1)).unwrap(), &16);
        assert_eq!(state.pools.get(&token).unwrap().get(&pr(2)), None);
    }

    #[test]
    fn test_limit_buying() {
        let mut state = State::default();
        state
            .pools
            .insert(MAINNET_LEDGER_CANISTER_ID, Default::default());
        state.tokens.insert(
            MAINNET_LEDGER_CANISTER_ID,
            Metadata {
                symbol: "ICP".into(),
                fee: DEFAULT_FEE.e8s(),
                decimals: 8,
                logo: None,
            },
        );

        state.revenue_account = Some(pr(255));

        let token = pr(100);

        state.add_token(token, "TAGGR".into(), 25, 2, None);

        // sell order for 7 $TAGGR / 0.05 ICP each
        state.add_liquidity(pr(0), token, 7).unwrap();
        assert!(state.create_order(pr(0), token, 7, 5000000, "sell").is_ok());

        // sell order for 16 $TAGGR / 0.03 ICP each
        state.add_liquidity(pr(1), token, 16).unwrap();
        assert!(state
            .create_order(pr(1), token, 16, 3000000, "sell")
            .is_ok());

        // sell order for 25 $TAGGR / 1 ICP each
        state.add_liquidity(pr(2), token, 25).unwrap();
        assert!(state
            .create_order(pr(2), token, 25, 100000000, "sell")
            .is_ok());

        // Order book: 16 @ 0.03, 7 @ 0.05, 25 @ 1

        let buyer = pr(5);

        state
            .add_liquidity(buyer, MAINNET_LEDGER_CANISTER_ID, 12 * 100000000)
            .unwrap();
        assert_eq!(
            state.trade("buy", buyer, token, 50, Some(6000000), 123456),
            Ok(())
        );

        // verify the partial filling
        let sell_orders = &state.orders.get(&token).unwrap().sellers;
        // we still have 1 order
        assert_eq!(sell_orders.len(), 1);
        let best_order = sell_orders.first().unwrap();
        // less tokens to buy at the given price as before
        assert_eq!(best_order.amount, 25);

        // buyer got 23 tokens
        assert_eq!(state.pools.get(&token).unwrap().get(&buyer).unwrap(), &23);

        // two sellers got ICP
        let (v2, v1) = (16 * 3000000, 7 * 5000000);
        assert_eq!(
            icp_pool(&state).get(&pr(0)).unwrap(),
            &(v1 - trading_fee(v1))
        );
        assert_eq!(
            icp_pool(&state).get(&pr(1)).unwrap(),
            &(v2 - trading_fee(v2))
        );
        assert_eq!(icp_pool(&state).get(&pr(2)), None);
    }
}
