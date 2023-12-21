use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet, VecDeque},
};

use candid::{CandidType, Principal};
use ic_ledger_types::MAINNET_LEDGER_CANISTER_ID;
use serde::{Deserialize, Serialize};

const PAYMENT_TOKEN_ID: Principal = MAINNET_LEDGER_CANISTER_ID;
pub type Timestamp = u64;
pub type Tokens = u128;
pub type TokenId = Principal;
pub type E8sPerToken = u128;
pub type E8s = u128;

pub const TX_FEE: u128 = 1; // 0.25% per trade side

#[derive(CandidType, Deserialize, PartialEq, Eq, Debug, Clone, Copy)]
pub enum OrderType {
    Buy,
    Sell,
}

impl OrderType {
    pub fn buy(&self) -> bool {
        self == &OrderType::Buy
    }
    pub fn sell(&self) -> bool {
        self == &OrderType::Sell
    }
}

#[derive(CandidType, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Order {
    owner: Principal,
    pub amount: Tokens,
    pub price: E8sPerToken,
    timestamp: Timestamp,
    pub executed: Timestamp,
}

impl Order {
    fn reserved_liquidity(&self, order_type: OrderType) -> Tokens {
        if order_type.buy() {
            let volume = self.amount * self.price;
            let fee = trading_fee(volume);
            volume + fee
        } else {
            self.amount
        }
    }
}

impl PartialOrd for Order {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Order {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.owner == other.owner
            && self.amount == other.amount
            && self.price == other.price
            && self.timestamp == other.timestamp
        {
            return Ordering::Equal;
        }
        if self.price == other.price
            && self.timestamp == other.timestamp
            && self.amount == other.amount
        {
            return self.owner.cmp(&other.owner);
        }
        if self.price == other.price && self.timestamp == other.timestamp {
            return self.amount.cmp(&other.amount);
        }
        if self.price == other.price {
            return self.timestamp.cmp(&other.timestamp);
        }
        self.price.cmp(&other.price)
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
struct Book {
    buyers: BTreeSet<Order>,
    sellers: BTreeSet<Order>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub symbol: String,
    pub fee: Tokens,
    pub decimals: u32,
    pub logo: Option<String>,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct State {
    orders: BTreeMap<TokenId, Book>,
    pub order_archive: BTreeMap<TokenId, VecDeque<Order>>,
    pools: BTreeMap<TokenId, BTreeMap<Principal, Tokens>>,
    pub tokens: BTreeMap<TokenId, Metadata>,
    pub e8s_per_xdr: u64,
    pub revenue_account: Option<Principal>,
    pub logs: VecDeque<String>,
}

impl State {
    pub fn payment_token_pool(&self) -> &BTreeMap<Principal, Tokens> {
        self.pools
            .get(&PAYMENT_TOKEN_ID)
            .expect("no payment token pool")
    }

    fn log(&mut self, message: String) {
        ic_cdk::println!("{}", &message);
        self.logs.push_front(message);
        while self.logs.len() > 1000 {
            self.logs.pop_back();
        }
    }

    pub fn close_order(
        &mut self,
        user: Principal,
        token: TokenId,
        amount: Tokens,
        price: E8sPerToken,
        timestamp: Timestamp,
        order_type: OrderType,
    ) -> Result<(), String> {
        let orders = self
            .orders
            .get_mut(&token)
            .map(|book| match order_type {
                OrderType::Buy => &mut book.buyers,
                OrderType::Sell => &mut book.sellers,
            })
            .ok_or("no token found")?;
        let order = Order {
            owner: user,
            price,
            amount,
            timestamp,
            executed: 0,
        };
        let reserved_liquidity = order.reserved_liquidity(order_type);
        if orders.remove(&order) {
            self.add_liquidity(
                user,
                if order_type.buy() {
                    PAYMENT_TOKEN_ID
                } else {
                    token
                },
                reserved_liquidity,
            )
        } else {
            Err("order not found".into())
        }
    }

    fn remove_orders(
        &mut self,
        token: TokenId,
        user: Principal,
        order_type: OrderType,
    ) -> Vec<Order> {
        let orders = match self.orders.get_mut(&token).map(|book| match order_type {
            OrderType::Buy => &mut book.buyers,
            OrderType::Sell => &mut book.sellers,
        }) {
            None => return Default::default(),
            Some(reference) => reference,
        };
        let result = orders
            .iter()
            .filter(|order| order.owner == user)
            .cloned()
            .collect();
        orders.retain(|order| order.owner != user);
        result
    }

    pub fn orders(
        &self,
        token: TokenId,
        order_type: OrderType,
    ) -> Box<dyn Iterator<Item = &'_ Order> + '_> {
        if let Some(book) = self.orders.get(&token) {
            match order_type {
                OrderType::Buy => Box::new(book.buyers.iter().rev()),
                OrderType::Sell => Box::new(book.sellers.iter()),
            }
        } else {
            Box::new(std::iter::empty())
        }
    }

    /// Returns liquidity for each listed token together with the liquidity locked in orders.
    pub fn token_balances(&self, user: Principal) -> BTreeMap<TokenId, (Tokens, Tokens)> {
        self.tokens
            .keys()
            .map(|token_id| {
                (
                    *token_id,
                    (
                        self.pools
                            .get(token_id)
                            .and_then(|pool| pool.get(&user).copied())
                            .unwrap_or_default(),
                        if token_id == &PAYMENT_TOKEN_ID {
                            self.orders
                                .values()
                                .flat_map(|book| {
                                    book.buyers.iter().filter_map(|order| {
                                        (order.owner == user)
                                            .then_some(order.reserved_liquidity(OrderType::Buy))
                                    })
                                })
                                .sum::<Tokens>()
                        } else {
                            self.orders
                                .get(token_id)
                                .map(|book| {
                                    book.sellers
                                        .iter()
                                        .filter_map(|order| {
                                            (order.owner == user).then_some(
                                                order.reserved_liquidity(OrderType::Sell),
                                            )
                                        })
                                        .sum::<Tokens>()
                                })
                                .unwrap_or_default()
                        },
                    ),
                )
            })
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
        user: Principal,
        id: TokenId,
        amount: Tokens,
    ) -> Result<(), String> {
        self.pools
            .get_mut(&id)
            .ok_or("token not found")?
            .entry(user)
            .and_modify(|funds| {
                *funds += amount;
            })
            .or_insert(amount);
        self.log(format!(
            "added {} tokens to {} pool for {}",
            amount, id, user,
        ));
        Ok(())
    }

    pub fn withdraw_liquidity(&mut self, user: Principal, id: TokenId) -> Result<Tokens, String> {
        let fee = self.tokens.get(&id).ok_or("no token found")?.fee;
        let pool = self.pools.get_mut(&id).ok_or("no token found")?;
        if fee > pool.get(&user).copied().unwrap_or_default() {
            return Err("amount smaller than transaction fee".into());
        }
        let mut result = pool
            .remove(&user)
            .map(|tokens| tokens.saturating_sub(fee))
            .ok_or("nothing to withdraw".to_string())?;

        let order_type = if id == PAYMENT_TOKEN_ID {
            OrderType::Buy
        } else {
            OrderType::Sell
        };

        let liquidity_in_orders: Tokens = self
            .remove_orders(id, user, order_type)
            .into_iter()
            .map(|order| order.amount)
            .sum();

        result += liquidity_in_orders;

        self.log(format!(
            "withdrew {} tokens from {} pool by {}",
            result, id, user,
        ));
        Ok(result)
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
        self.log(format!("token {} was listed", id));
    }

    pub fn create_order(
        &mut self,
        user: Principal,
        token: TokenId,
        amount: Tokens,
        price: E8sPerToken,
        timestamp: Timestamp,
        order_type: OrderType,
    ) -> Result<(), String> {
        if !self.tokens.contains_key(&token) {
            return Err("token not listed".into());
        }

        let order = Order {
            owner: user,
            amount,
            price,
            timestamp,
            executed: 0,
        };
        let order_book = self.orders.entry(token).or_default();
        let token_balance = self
            .pools
            .get_mut(&if order_type.buy() {
                PAYMENT_TOKEN_ID
            } else {
                token
            })
            .ok_or("no token found")?
            .get_mut(&user)
            .ok_or("no funds available")?;
        let required_liquidity = order.reserved_liquidity(order_type);
        if required_liquidity > *token_balance {
            return Err("not enough funds available for this order size".into());
        }

        if !(if order_type.buy() {
            order_book.buyers.insert(order)
        } else {
            order_book.sellers.insert(order)
        }) {
            return Err("order exists already".into());
        }

        *token_balance = token_balance.saturating_sub(required_liquidity);
        self.log(format!(
            "{} created {:?} order for {} {} at limit price {}",
            user, order_type, amount, token, price
        ));
        Ok(())
    }

    pub fn trade(
        &mut self,
        trade_type: OrderType,
        trader: Principal,
        token: TokenId,
        mut amount: u128,
        limit: Option<E8sPerToken>,
        time: Timestamp,
    ) -> Result<u128, String> {
        let book = &mut match self.orders.get_mut(&token) {
            Some(order_book) => order_book,
            _ => return Ok(0),
        };

        let orders = if trade_type.buy() {
            &mut book.sellers
        } else {
            &mut book.buyers
        };

        let archive = self.order_archive.entry(token).or_default();

        let mut filled = 0;
        while let Some(mut order) = if trade_type.buy() {
            orders.pop_first()
        } else {
            orders.pop_last()
        } {
            // limit checks
            if trade_type.buy() && limit.is_some() && limit < Some(order.price)
                || trade_type.sell() && limit > Some(order.price)
            {
                orders.insert(order);
                break;
            }

            amount = if order.amount > amount {
                // partial order fill - create a new one for left overs
                let mut remaining_order = order.clone();
                remaining_order.amount = order.amount - amount;
                orders.insert(remaining_order);
                order.amount = amount;
                0
            } else {
                amount - order.amount
            };

            adjust_pools(
                &mut self.pools,
                trader,
                token,
                &order,
                self.revenue_account.unwrap(),
                trade_type,
            )?;

            filled += order.amount;
            order.executed = time;
            order.owner = Principal::anonymous();
            archive.push_front(order);

            if amount == 0 {
                break;
            }
        }

        if filled > 0 {
            self.log(format!(
                "{} {} {} {} with the limit price {:?}",
                trader,
                if trade_type.buy() { "bought" } else { "sold" },
                filled,
                token,
                limit
            ));
        }

        Ok(filled)
    }
}

fn adjust_pools(
    pools: &mut BTreeMap<TokenId, BTreeMap<Principal, Tokens>>,
    trader: Principal,
    token: TokenId,
    order: &Order,
    revenue_account: Principal,
    // since the liquidity is locked inside the order, we need to know where we should avoid
    // adjusting pools
    trade_type: OrderType,
) -> Result<(), String> {
    let (payment_receiver, token_receiver) = if trade_type.buy() {
        (order.owner, trader)
    } else {
        (trader, order.owner)
    };

    let token_pool = pools.get_mut(&token).ok_or("no token pool found")?;
    // We only need to subtract token liquidity if we're executing a selling trade, becasue we
    // process buy orders
    if trade_type.sell() {
        let sellers_tokens = token_pool.entry(payment_receiver).or_insert(0);
        *sellers_tokens = sellers_tokens
            .checked_sub(order.amount)
            .ok_or("not enough tokens")?;
    }

    let buyers_tokens = token_pool.entry(token_receiver).or_insert(0);
    *buyers_tokens += order.amount;

    let icp_pool = pools
        .get_mut(&PAYMENT_TOKEN_ID)
        .ok_or("no icp pool found")?;
    let volume = order.amount * order.price;
    let fee = trading_fee(volume);

    // We only need to subtract payment liquidity if we're executing a buying trade, becasue we
    // process sell orders
    if trade_type.buy() {
        let buyers_icp_tokens = icp_pool.get_mut(&token_receiver).ok_or("no ICP tokens")?;
        *buyers_icp_tokens = buyers_icp_tokens
            .checked_sub(volume + fee)
            .ok_or("not enough ICP tokens")?;
    }

    let sellers_icp_tokens = icp_pool.entry(payment_receiver).or_default();
    *sellers_icp_tokens += volume.checked_sub(fee).ok_or("amount smaller than fees")?;
    let icp_fees = icp_pool.entry(revenue_account).or_default();
    *icp_fees += 2 * fee;
    Ok(())
}

fn trading_fee(volume: E8s) -> u128 {
    volume * TX_FEE / 10000
}

#[cfg(test)]
mod tests {

    use ic_ledger_types::DEFAULT_FEE;

    use super::*;

    pub fn pr(n: u8) -> Principal {
        let v = vec![0, n];
        Principal::from_slice(&v)
    }
    fn user_orders(
        state: &State,
        token: TokenId,
        user: Principal,
        order_type: OrderType,
    ) -> Box<dyn Iterator<Item = &'_ Order> + '_> {
        Box::new(
            state
                .orders(token, order_type)
                .filter(move |order| order.owner == user),
        )
    }

    #[test]
    fn test_orderbook() {
        let mut o1 = Order {
            owner: pr(16),
            amount: 12,
            price: 0,
            timestamp: 111,
            executed: 0,
        };
        let mut o2 = Order {
            owner: pr(16),
            amount: 32,
            price: 0,
            timestamp: 111,
            executed: 0,
        };

        assert_eq!(o1.cmp(&o1), Ordering::Equal);
        assert_eq!(o1.cmp(&o2), Ordering::Less);
        o1.amount = o2.amount;
        assert_eq!(o1.cmp(&o2), Ordering::Equal);
        o2.owner = pr(1);
        o2.amount = 3000;
        o2.price = 2;
        o1.price = 3;
        assert_eq!(o2.cmp(&o1), Ordering::Less);
        let mut o3 = o1.clone();
        assert_eq!(o3.cmp(&o1), Ordering::Equal);
        o3.timestamp += 1;
        assert_eq!(o3.cmp(&o1), Ordering::Greater);
    }

    #[test]
    fn test_order_closing() {
        {
            let mut state = State::default();
            state.pools.insert(PAYMENT_TOKEN_ID, Default::default());
            state.tokens.insert(
                PAYMENT_TOKEN_ID,
                Metadata {
                    symbol: "ICP".into(),
                    fee: DEFAULT_FEE.e8s() as u128,
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

            assert_eq!(state.token_balances(pr(0)).get(&token).unwrap().0, 333);

            assert_eq!(
                state.create_order(pr(0), token, 250, 0, 0, OrderType::Sell),
                Ok(())
            );
            assert_eq!(
                state.create_order(pr(0), token, 50, 0, 0, OrderType::Sell),
                Ok(())
            );

            assert_eq!(
                state.token_balances(pr(0)).get(&token).unwrap().0,
                333 - 250 - 50
            );
            assert_eq!(
                state.close_order(pr(0), token, 50, 0, 0, OrderType::Sell),
                Ok(())
            );
            assert_eq!(
                state.close_order(pr(0), token, 250, 0, 0, OrderType::Sell),
                Ok(())
            );
            assert_eq!(state.token_balances(pr(0)).get(&token).unwrap().0, 333);

            state
                .add_liquidity(pr(0), PAYMENT_TOKEN_ID, 8 * 10000000)
                .unwrap();
            assert!(state
                .create_order(pr(0), token, 3, 10000000, 0, OrderType::Buy)
                .is_ok());

            let volume = 3 * 10000000;

            assert_eq!(
                state
                    .token_balances(pr(0))
                    .get(&PAYMENT_TOKEN_ID)
                    .copied()
                    .unwrap()
                    .0,
                8 * 10000000 - volume - trading_fee(volume)
            );

            assert_eq!(
                state.create_order(pr(0), token, 3, 10000000, 0, OrderType::Buy),
                Err("order exists already".into())
            );

            assert!(state
                .create_order(pr(0), token, 4, 10000000, 0, OrderType::Buy)
                .is_ok());

            let volume2 = 4 * 10000000;
            assert_eq!(
                state
                    .token_balances(pr(0))
                    .get(&PAYMENT_TOKEN_ID)
                    .copied()
                    .unwrap()
                    .0,
                8 * 10000000 - volume - trading_fee(volume) - volume2 - trading_fee(volume2)
            );
            assert_eq!(
                state.close_order(pr(0), token, 3, 10000000, 0, OrderType::Buy),
                Ok(())
            );
            assert_eq!(
                state.close_order(pr(0), token, 4, 10000000, 0, OrderType::Buy),
                Ok(())
            );
            assert_eq!(
                state
                    .token_balances(pr(0))
                    .get(&PAYMENT_TOKEN_ID)
                    .copied()
                    .unwrap()
                    .0,
                8 * 10000000
            );
        }
    }

    #[test]
    fn test_liquidity_adding_and_withdrawals() {
        let mut state = State::default();
        state.pools.insert(PAYMENT_TOKEN_ID, Default::default());
        state.tokens.insert(
            PAYMENT_TOKEN_ID,
            Metadata {
                symbol: "ICP".into(),
                fee: DEFAULT_FEE.e8s() as u128,
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

        assert_eq!(state.token_balances(pr(0)).get(&token).unwrap().0, 333);

        assert_eq!(
            state.create_order(pr(0), token, 250, 0, 0, OrderType::Sell),
            Ok(())
        );

        assert_eq!(
            state.token_balances(pr(0)).get(&token).unwrap().0,
            333 - 250
        );

        assert_eq!(user_orders(&state, token, pr(0), OrderType::Buy).count(), 0);
        let sell_orders = user_orders(&state, token, pr(0), OrderType::Sell).collect::<Vec<_>>();
        assert_eq!(sell_orders.len(), 1);
        assert_eq!(sell_orders.first().unwrap().amount, 250);

        assert_eq!(
            state.withdraw_liquidity(pr(1), token),
            Err("amount smaller than transaction fee".into())
        );
        assert_eq!(state.withdraw_liquidity(pr(0), token), Ok(333 - 25));

        let sell_orders = user_orders(&state, token, pr(0), OrderType::Sell).collect::<Vec<_>>();
        assert_eq!(sell_orders.len(), 0);

        let one_icp = 100000000;
        state
            .add_liquidity(pr(0), PAYMENT_TOKEN_ID, one_icp)
            .unwrap();
        assert_eq!(
            state.withdraw_liquidity(pr(1), PAYMENT_TOKEN_ID),
            Err("amount smaller than transaction fee".into())
        );
        assert_eq!(
            state.withdraw_liquidity(pr(0), PAYMENT_TOKEN_ID),
            Ok((ic_ledger_types::Tokens::from_e8s(one_icp as u64) - DEFAULT_FEE).e8s() as u128)
        );
    }

    #[test]
    fn test_selling() {
        let mut state = State::default();
        state.pools.insert(PAYMENT_TOKEN_ID, Default::default());
        state.tokens.insert(
            PAYMENT_TOKEN_ID,
            Metadata {
                symbol: "ICP".into(),
                fee: DEFAULT_FEE.e8s() as u128,
                decimals: 8,
                logo: None,
            },
        );

        state.revenue_account = Some(pr(255));

        let token = pr(100);

        assert_eq!(
            state.create_order(pr(0), token, 7, 50000000, 0, OrderType::Buy),
            Err("token not listed".into())
        );

        state.add_token(token, "TAGGR".into(), 25, 2, None);

        // buy order for 7 $TAGGR / 0.1 ICP each
        assert_eq!(
            state.create_order(pr(0), token, 7, 10000000, 0, OrderType::Buy),
            Err("no funds available".into())
        );

        state
            .add_liquidity(pr(0), PAYMENT_TOKEN_ID, 8 * 10000000)
            .unwrap();
        assert!(state
            .create_order(pr(0), token, 7, 10000000, 0, OrderType::Buy)
            .is_ok());

        // buy order for 16 $TAGGR / 0.03 ICP each
        state
            .add_liquidity(pr(1), PAYMENT_TOKEN_ID, 17 * 30000000)
            .unwrap();
        assert!(state
            .create_order(pr(1), token, 16, 3000000, 0, OrderType::Buy)
            .is_ok());

        // buy order for 25 $TAGGR / 0.01 ICP each
        state
            .add_liquidity(pr(2), PAYMENT_TOKEN_ID, 24 * 1000000)
            .unwrap();
        assert_eq!(
            state.create_order(pr(2), token, 25, 1000000, 0, OrderType::Buy),
            Err("not enough funds available for this order size".into())
        );
        state
            .add_liquidity(pr(2), PAYMENT_TOKEN_ID, 2 * 1000000)
            .unwrap();
        assert!(state
            .create_order(pr(2), token, 25, 1000000, 0, OrderType::Buy)
            .is_ok());

        // buyer has 0.01 ICP left minus fee
        assert_eq!(state.payment_token_pool().get(&pr(2)).unwrap(), &937500);

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
        assert_eq!(state.payment_token_pool().len(), 3);
        let buyer_orders = &state.orders.get(&token).unwrap().buyers;
        // we have 3 orders
        assert_eq!(buyer_orders.len(), 3);

        let seller = pr(5);

        assert_eq!(
            state
                .clone()
                .trade(OrderType::Sell, seller, token, 5, None, 123456),
            Err("not enough tokens".into())
        );
        state.add_liquidity(seller, token, 250).unwrap();
        assert_eq!(
            state.trade(OrderType::Sell, seller, token, 5, None, 123456),
            Ok(5)
        );

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
        let executed_order = archived_orders.front().unwrap();
        assert_eq!(executed_order.executed, 123456);
        // only 5 tokens got traded
        assert_eq!(executed_order.amount, 5);

        // buyer got 5 tokens
        assert_eq!(state.pools.get(&token).unwrap().get(&pr(0)).unwrap(), &5);

        // now seller should get a balance too, plus the fee acount
        assert_eq!(state.payment_token_pool().len(), 5);
        // seller has expected amount of ICP: 5 * 0.1 ICP - fee
        let volume = 50000000;
        let fee_per_side = trading_fee(volume);
        assert_eq!(
            state.payment_token_pool().get(&seller).unwrap(),
            &(volume - fee_per_side)
        );
        // buyer should have previous amount - volume - fee;
        assert_eq!(state.payment_token_pool().get(&pr(0)).unwrap(), &9825000);
        // fee account has 2 fees
        assert_eq!(
            state.payment_token_pool().get(&pr(255)).unwrap(),
            &(2 * fee_per_side)
        );

        // let's sell more
        // at that point we have buy orders: 25 @ 0.01, 16 @ 0.03, 2 @ 0.1
        let buyer_orders = &state.orders.get(&token).unwrap().buyers;
        assert_eq!(buyer_orders.len(), 3);
        let best_order = buyer_orders.last().unwrap();
        assert_eq!(best_order.amount, 2);
        assert_eq!(best_order.price, 10000000);

        assert_eq!(
            state.trade(OrderType::Sell, seller, token, 10, None, 123457),
            Ok(10)
        );

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

        // at that point we have buy orders: 25 @ 0.01, 8 @ 0.03
        assert_eq!(
            state.trade(OrderType::Sell, seller, token, 150, None, 123457),
            Ok(33)
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
            state.payment_token_pool().get(&seller).unwrap(),
            &(v1 + v2 + v3 - fees)
        );
        assert_eq!(
            state.payment_token_pool().get(&pr(255)).unwrap(),
            &(2 * fees)
        );
    }

    #[test]
    fn test_buying() {
        let mut state = State::default();
        state.pools.insert(PAYMENT_TOKEN_ID, Default::default());
        state.tokens.insert(
            PAYMENT_TOKEN_ID,
            Metadata {
                symbol: "ICP".into(),
                fee: DEFAULT_FEE.e8s() as u128,
                decimals: 8,
                logo: None,
            },
        );

        state.revenue_account = Some(pr(255));

        let token = pr(100);

        assert_eq!(
            state
                .clone()
                .create_order(pr(0), token, 7, 5000000, 0, OrderType::Sell),
            Err("token not listed".into())
        );

        state.add_token(token, "TAGGR".into(), 25, 2, None);

        // sell order for 7 $TAGGR / 0.05 ICP each
        assert_eq!(
            state
                .clone()
                .create_order(pr(0), token, 7, 5000000, 0, OrderType::Sell),
            Err("no funds available".into())
        );

        state.add_liquidity(pr(0), token, 7).unwrap();
        assert!(state
            .create_order(pr(0), token, 7, 5000000, 0, OrderType::Sell)
            .is_ok());

        // sell order for 16 $TAGGR / 0.03 ICP each
        state.add_liquidity(pr(1), token, 16).unwrap();
        assert!(state
            .create_order(pr(1), token, 16, 3000000, 0, OrderType::Sell)
            .is_ok());

        // sell order for 25 $TAGGR / 1 ICP each
        state.add_liquidity(pr(2), token, 24).unwrap();
        assert_eq!(
            state
                .clone()
                .create_order(pr(2), token, 25, 100000000, 0, OrderType::Sell),
            Err("not enough funds available for this order size".into())
        );
        state.add_liquidity(pr(2), token, 1).unwrap();
        assert!(state
            .create_order(pr(2), token, 25, 100000000, 0, OrderType::Sell)
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

        // since we had no ICP we didn't buy anything
        assert_eq!(state.pools.get(&token).unwrap().get(&buyer), None);
        state
            .add_liquidity(buyer, PAYMENT_TOKEN_ID, 12 * 3000000)
            .unwrap();
        assert_eq!(state.payment_token_pool().len(), 1);
        assert_eq!(
            state.trade(OrderType::Buy, buyer, token, 10, None, 123456),
            Ok(10)
        );

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
        let executed_order = archived_orders.front().unwrap();
        assert_eq!(executed_order.executed, 123456);
        // only 5 tokens got traded
        assert_eq!(executed_order.amount, 10);

        // buyer got 10 tokens
        assert_eq!(state.pools.get(&token).unwrap().get(&buyer).unwrap(), &10);

        // now seller should get a balance too, plus the fee acount
        assert_eq!(state.payment_token_pool().len(), 3);

        // let's buy more
        // at that point we have buy orders: 6 @ 0.03, 7 @ 0.05, 25 @ 1
        state
            .add_liquidity(buyer, PAYMENT_TOKEN_ID, 6 * 3000000 + 2 * 5000000)
            .unwrap();
        assert_eq!(
            state.trade(OrderType::Buy, buyer, token, 7, None, 123457),
            Ok(7)
        );
        // buyer got 17 tokens
        assert_eq!(state.pools.get(&token).unwrap().get(&buyer).unwrap(), &17);

        // we should have only two now
        let sell_orders = &state.orders.get(&token).unwrap().sellers;
        assert_eq!(sell_orders.len(), 2);
        let best_order = sell_orders.first().unwrap();
        assert_eq!(best_order.amount, 6);
        assert_eq!(best_order.price, 5000000);

        state
            .add_liquidity(buyer, PAYMENT_TOKEN_ID, 6 * 5000000 + 28 * 100000000)
            .unwrap();

        assert_eq!(
            state.trade(OrderType::Buy, buyer, token, 100, None, 123458),
            Ok(31)
        );

        // all sellers got ICP
        let (v2, v1, v3) = (16 * 3000000, 7 * 5000000, 25 * 100000000);
        assert_eq!(
            state.payment_token_pool().get(&pr(0)).unwrap(),
            &(v1 - trading_fee(v1))
        );
        assert_eq!(
            state.payment_token_pool().get(&pr(1)).unwrap(),
            &(v2 - trading_fee(v2))
        );
        assert_eq!(
            state.payment_token_pool().get(&pr(2)).unwrap(),
            &(v3 - trading_fee(v3))
        );

        // executed orders: 16 @ 0.03, 7 @ 0.05, 25 @ 1
        let fees = trading_fee(v1) + trading_fee(v2) + trading_fee(v3);
        assert_eq!(
            state.payment_token_pool().get(&pr(255)).unwrap(),
            &(2 * fees)
        );
    }

    #[test]
    fn test_limit_selling() {
        let mut state = State::default();
        state.pools.insert(PAYMENT_TOKEN_ID, Default::default());
        state.tokens.insert(
            PAYMENT_TOKEN_ID,
            Metadata {
                symbol: "ICP".into(),
                fee: DEFAULT_FEE.e8s() as u128,
                decimals: 8,
                logo: None,
            },
        );

        state.revenue_account = Some(pr(255));

        let token = pr(100);

        state.add_token(token, "TAGGR".into(), 25, 2, None);

        // buy order for 7 $TAGGR / 0.1 ICP each
        state
            .add_liquidity(pr(0), PAYMENT_TOKEN_ID, 8 * 10000000)
            .unwrap();
        assert!(state
            .create_order(pr(0), token, 7, 10000000, 0, OrderType::Buy)
            .is_ok());

        // buy order for 16 $TAGGR / 0.03 ICP each
        state
            .add_liquidity(pr(1), PAYMENT_TOKEN_ID, 17 * 30000000)
            .unwrap();
        assert!(state
            .create_order(pr(1), token, 16, 3000000, 0, OrderType::Buy)
            .is_ok());

        // buy order for 25 $TAGGR / 0.01 ICP each
        state
            .add_liquidity(pr(2), PAYMENT_TOKEN_ID, 26 * 1000000)
            .unwrap();
        assert!(state
            .create_order(pr(2), token, 25, 1000000, 0, OrderType::Buy)
            .is_ok());

        // Orer book: 7 @ 0.1, 16 @ 0.03, 25 @ 0.01

        let seller = pr(5);

        state.add_liquidity(seller, token, 250).unwrap();
        assert_eq!(
            state.trade(OrderType::Sell, seller, token, 50, Some(2000000), 123456),
            Ok(23)
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
        state.pools.insert(PAYMENT_TOKEN_ID, Default::default());
        state.tokens.insert(
            PAYMENT_TOKEN_ID,
            Metadata {
                symbol: "ICP".into(),
                fee: DEFAULT_FEE.e8s() as u128,
                decimals: 8,
                logo: None,
            },
        );

        state.revenue_account = Some(pr(255));

        let token = pr(100);

        state.add_token(token, "TAGGR".into(), 25, 2, None);

        // sell order for 7 $TAGGR / 0.05 ICP each
        state.add_liquidity(pr(0), token, 7).unwrap();
        assert!(state
            .create_order(pr(0), token, 7, 5000000, 0, OrderType::Sell)
            .is_ok());

        // sell order for 16 $TAGGR / 0.03 ICP each
        state.add_liquidity(pr(1), token, 16).unwrap();
        assert!(state
            .create_order(pr(1), token, 16, 3000000, 0, OrderType::Sell)
            .is_ok());

        // sell order for 25 $TAGGR / 1 ICP each
        state.add_liquidity(pr(2), token, 25).unwrap();
        assert!(state
            .create_order(pr(2), token, 25, 100000000, 0, OrderType::Sell)
            .is_ok());

        // Order book: 16 @ 0.03, 7 @ 0.05, 25 @ 1

        let buyer = pr(5);

        state
            .add_liquidity(buyer, PAYMENT_TOKEN_ID, 12 * 100000000)
            .unwrap();
        assert_eq!(
            state.trade(OrderType::Buy, buyer, token, 50, Some(6000000), 123456),
            Ok(23)
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
            state.payment_token_pool().get(&pr(0)).unwrap(),
            &(v1 - trading_fee(v1))
        );
        assert_eq!(
            state.payment_token_pool().get(&pr(1)).unwrap(),
            &(v2 - trading_fee(v2))
        );
        assert_eq!(state.payment_token_pool().get(&pr(2)), None);
    }

    #[test]
    fn test_liquitidy_lock() {
        let mut state = State::default();
        state.pools.insert(PAYMENT_TOKEN_ID, Default::default());
        state.tokens.insert(
            PAYMENT_TOKEN_ID,
            Metadata {
                symbol: "ICP".into(),
                fee: DEFAULT_FEE.e8s() as u128,
                decimals: 8,
                logo: None,
            },
        );

        state.revenue_account = Some(pr(255));

        let token = pr(100);

        state.add_token(token, "TAGGR".into(), 25, 2, None);

        // sell order for 7 $TAGGR / 0.05 ICP each
        state.add_liquidity(pr(0), token, 7).unwrap();
        assert!(state
            .create_order(pr(0), token, 7, 5000000, 0, OrderType::Sell)
            .is_ok());
        assert_eq!(
            state.create_order(pr(0), token, 7, 6000000, 0, OrderType::Sell),
            Err("not enough funds available for this order size".into())
        );
    }
}
