use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet, VecDeque},
};

use candid::{CandidType, Principal};
use ic_ledger_types::{DEFAULT_FEE, MAINNET_LEDGER_CANISTER_ID};
use serde::{Deserialize, Serialize};

use crate::icrc1::Value;

pub const PAYMENT_TOKEN_ID: Principal = MAINNET_LEDGER_CANISTER_ID;
pub type Timestamp = u64;
pub type Tokens = u128;
pub type TokenId = Principal;
pub type E8sPerToken = u128;
pub type E8s = u128;

pub const TX_FEE: u128 = 1; // 0.25% per trade side

#[derive(CandidType, Serialize, Deserialize, PartialEq, Eq, Debug, Clone, Copy)]
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
    order_type: OrderType,
    owner: Principal,
    pub amount: Tokens,
    pub price: E8sPerToken,
    timestamp: Timestamp,
    pub executed: Timestamp,
}

impl Order {
    fn reserved_liquidity(&self) -> Tokens {
        if self.order_type.buy() {
            let volume = self.amount * self.price;
            volume + trading_fee(volume)
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
    pub logs: VecDeque<(u64, String)>,
    event_id: u64,
}

impl State {
    pub fn list_token(
        &mut self,
        token: TokenId,
        metadata: BTreeMap<String, Value>,
    ) -> Result<(), String> {
        match (
            metadata.get("icrc1:symbol"),
            metadata.get("icrc1:fee"),
            metadata.get("icrc1:decimals"),
            metadata.get("icrc1:logo"),
        ) {
            (
                Some(Value::Text(symbol)),
                Some(Value::Nat(fee)),
                Some(Value::Nat(decimals)),
                logo,
            ) => {
                self.add_token(
                    token,
                    symbol.clone(),
                    *fee,
                    *decimals as u32,
                    match logo {
                        Some(Value::Text(hex)) => Some(hex.clone()),
                        _ => None,
                    },
                );
                Ok(())
            }
            (symbol, fee, decimals, _) => Err(format!(
                "one of the required values missing: symbol={:?}, fee={:?}, decimals={:?}",
                symbol, fee, decimals
            )),
        }
    }

    pub fn token_pool_balance(&self, token: TokenId, user: Principal) -> Tokens {
        self.pools
            .get(&token)
            .and_then(|pool| pool.get(&user).copied())
            .unwrap_or_default()
    }

    pub fn payment_token_pool(&self) -> &BTreeMap<Principal, Tokens> {
        self.pools
            .get(&PAYMENT_TOKEN_ID)
            .expect("no payment token pool")
    }

    pub fn log(&mut self, message: String) {
        ic_cdk::println!("{}", &message);
        let event_id = self.event_id;
        self.event_id += 1;
        self.logs.push_front((event_id, message));
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
            order_type,
            owner: user,
            price,
            amount,
            timestamp,
            executed: 0,
        };
        let reserved_liquidity = order.reserved_liquidity();
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
                                        (order.owner == user).then_some(order.reserved_liquidity())
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
                                            (order.owner == user)
                                                .then_some(order.reserved_liquidity())
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

    pub fn logs(&self) -> &VecDeque<(u64, String)> {
        &self.logs
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

    pub fn charge(&mut self, user: Principal, amount: Tokens) -> Result<(), String> {
        let payment_token_pool = self
            .pools
            .get_mut(&PAYMENT_TOKEN_ID)
            .ok_or("token not found")?;
        let balance = payment_token_pool.entry(user).or_insert(0);
        *balance = balance.checked_sub(amount).ok_or("not enough funds")?;
        payment_token_pool
            .entry(self.revenue_account.expect("no revenue account set"))
            .and_modify(|balance| *balance += amount)
            .or_insert(amount);

        self.log(format!("{} paid {} tokens", user, amount));
        Ok(())
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
        let pool = self.pools.get_mut(&id).ok_or("no token found")?;
        let amount = pool
            .remove(&user)
            .ok_or("nothing to withdraw".to_string())?;
        self.log(format!(
            "withdrew {} tokens from {} pool by {}",
            amount, id, user,
        ));
        Ok(amount)
    }

    fn add_token(
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
        if price == 0 {
            return Err("limit price is 0".into());
        }
        if !self.tokens.contains_key(&token) {
            return Err("token not listed".into());
        }

        let order = Order {
            order_type,
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
        let required_liquidity = order.reserved_liquidity();
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
        user: Principal,
        token: TokenId,
        amount: u128,
        price: E8sPerToken,
        now: Timestamp,
    ) -> Result<(u128, bool), String> {
        // match existing orders
        let filled = self.execute_trade(
            trade_type,
            user,
            token,
            amount,
            (price > 0).then_some(price),
            now,
        )?;

        // create a rest order if the original was not filled and this was a limit order
        Ok((
            filled,
            if filled < amount && price > 0 {
                self.create_order(
                    user,
                    token,
                    amount.saturating_sub(filled),
                    price,
                    now,
                    trade_type,
                )?;
                true
            } else {
                false
            },
        ))
    }

    fn execute_trade(
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

    /// This method is used for an invariance check, making sure that no funds get lost.
    /// It returns a simple mapping from the token id, to the amount of managed funds.
    ///
    /// Note, that additionally to unlocked liquidity, we need to count all funds locked in
    /// buying orders for the payment token, and all funds locked in sell orders of
    /// a non-payment token
    pub fn funds_under_management(&self) -> Vec<(String, Tokens)> {
        self.pools
            .iter()
            .map(|(id, pool)| {
                (
                    id.to_string(),
                    pool.values().sum::<Tokens>()
                        + if id == &PAYMENT_TOKEN_ID {
                            self.orders
                                .values()
                                .flat_map(|book| {
                                    book.buyers.iter().map(|order| order.reserved_liquidity())
                                })
                                .sum::<Tokens>()
                        } else {
                            self.orders
                                .get(id)
                                .map(|book| {
                                    book.sellers
                                        .iter()
                                        .map(|order| order.reserved_liquidity())
                                        .sum::<Tokens>()
                                })
                                .unwrap_or_default()
                        },
                )
            })
            .collect()
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

    let payment_token_pool = pools
        .get_mut(&PAYMENT_TOKEN_ID)
        .ok_or("no payment pool found")?;

    let volume = order.amount * order.price;
    let fee = trading_fee(volume);

    // We only need to subtract payment liquidity if we're executing a buying trade, becasue we
    // process sell orders
    if trade_type.buy() {
        let buyers_payment_tokens = payment_token_pool
            .get_mut(&token_receiver)
            .ok_or("no payment tokens")?;
        *buyers_payment_tokens = buyers_payment_tokens
            .checked_sub(volume + fee)
            .ok_or("not enough payment tokens")?;
    }

    let sellers_payment_tokens = payment_token_pool.entry(payment_receiver).or_default();
    *sellers_payment_tokens += volume.checked_sub(fee).ok_or("amount smaller than fee")?;
    let payment_fees = payment_token_pool.entry(revenue_account).or_default();
    *payment_fees += 2 * fee;
    Ok(())
}

fn trading_fee(volume: E8s) -> u128 {
    volume * TX_FEE / DEFAULT_FEE.e8s() as u128
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

    fn close_order(
        state: &mut State,
        user: Principal,
        token: TokenId,
        amount: Tokens,
        price: E8sPerToken,
        timestamp: Timestamp,
        order_type: OrderType,
    ) -> Result<(), String> {
        let fum = state.funds_under_management();
        let result = state.close_order(user, token, amount, price, timestamp, order_type);
        if result.is_ok() {
            assert_eq!(fum, state.funds_under_management());
        }
        result
    }

    fn create_order(
        state: &mut State,
        user: Principal,
        token: TokenId,
        amount: Tokens,
        price: E8sPerToken,
        timestamp: Timestamp,
        order_type: OrderType,
    ) -> Result<(), String> {
        let fum = state.funds_under_management();
        let result = state.create_order(user, token, amount, price, timestamp, order_type);
        if result.is_ok() {
            assert_eq!(fum, state.funds_under_management());
        }
        result
    }

    fn trade(
        state: &mut State,
        trade_type: OrderType,
        trader: Principal,
        token: TokenId,
        amount: u128,
        limit: Option<E8sPerToken>,
        time: Timestamp,
    ) -> Result<u128, String> {
        let fum = state.funds_under_management();
        let result = state.execute_trade(trade_type, trader, token, amount, limit, time);
        if result.is_ok() {
            assert_eq!(fum, state.funds_under_management());
        }
        result
    }

    fn list_payment_token(state: &mut State) {
        state.revenue_account = Some(pr(255));
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
    }

    #[test]
    fn test_orderbook() {
        let mut o1 = Order {
            order_type: OrderType::Buy,
            owner: pr(16),
            amount: 12,
            price: 0,
            timestamp: 111,
            executed: 0,
        };
        let mut o2 = Order {
            order_type: OrderType::Buy,
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
    fn test_simplest_trade() {
        let state = &mut State::default();
        list_payment_token(state);
        let token = pr(100);

        state.add_token(
            token,
            "TAGGR".into(),
            25, // fee
            2,  // decimals
            None,
        );

        state.add_liquidity(pr(1), PAYMENT_TOKEN_ID, 21000).unwrap();
        assert_eq!(trading_fee(20000), 2);
        assert_eq!(
            create_order(state, pr(1), token, 1, 0, 0, OrderType::Buy),
            Err("limit price is 0".into())
        );
        assert_eq!(
            create_order(state, pr(1), token, 1, 22000, 0, OrderType::Buy),
            Err("not enough funds available for this order size".into())
        );

        assert_eq!(
            create_order(state, pr(1), token, 1, 21000, 0, OrderType::Buy),
            Err("not enough funds available for this order size".into())
        );

        assert_eq!(
            create_order(state, pr(1), token, 1, 20000, 0, OrderType::Buy),
            Ok(())
        );

        state.add_liquidity(pr(0), token, 1).unwrap();
        assert_eq!(
            trade(state, OrderType::Sell, pr(0), token, 1, None, 123456),
            Ok(1)
        );
    }

    #[test]
    fn test_order_closing() {
        let state = &mut State::default();
        list_payment_token(state);

        let token = pr(100);

        assert_eq!(
            state.add_liquidity(pr(0), token, 111),
            Err("token not found".into())
        );

        state.add_token(
            token,
            "TAGGR".into(),
            25, // fee
            2,  // decimals
            None,
        );

        state.add_liquidity(pr(0), token, 111).unwrap();
        state.add_liquidity(pr(0), token, 222).unwrap();

        assert_eq!(state.token_balances(pr(0)).get(&token).unwrap().0, 333);

        assert_eq!(
            create_order(state, pr(0), token, 250, 1, 0, OrderType::Sell),
            Ok(())
        );
        assert_eq!(
            create_order(state, pr(0), token, 50, 1, 0, OrderType::Sell),
            Ok(())
        );

        assert_eq!(
            state.token_balances(pr(0)).get(&token).unwrap().0,
            333 - 250 - 50
        );
        assert_eq!(
            close_order(state, pr(0), token, 50, 1, 0, OrderType::Sell),
            Ok(())
        );
        assert_eq!(
            close_order(state, pr(0), token, 250, 1, 0, OrderType::Sell),
            Ok(())
        );
        assert_eq!(state.token_balances(pr(0)).get(&token).unwrap().0, 333);

        state
            .add_liquidity(pr(0), PAYMENT_TOKEN_ID, 8 * 10000000)
            .unwrap();
        assert!(create_order(state, pr(0), token, 3, 10000000, 0, OrderType::Buy).is_ok());

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
            create_order(state, pr(0), token, 3, 10000000, 0, OrderType::Buy),
            Err("order exists already".into())
        );

        assert!(create_order(state, pr(0), token, 4, 10000000, 0, OrderType::Buy).is_ok());

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
            close_order(state, pr(0), token, 3, 10000000, 0, OrderType::Buy),
            Ok(())
        );
        assert_eq!(
            close_order(state, pr(0), token, 4, 10000000, 0, OrderType::Buy),
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

    #[test]
    fn test_liquidity_adding_and_withdrawals() {
        let state = &mut State::default();
        list_payment_token(state);

        let token = pr(100);

        assert_eq!(
            state.add_liquidity(pr(0), token, 111),
            Err("token not found".into())
        );

        state.add_token(
            token,
            "TAGGR".into(),
            25, // fee
            2,  // decimals
            None,
        );

        state.add_liquidity(pr(0), token, 111).unwrap();
        state.add_liquidity(pr(0), token, 222).unwrap();

        assert_eq!(state.token_balances(pr(0)).get(&token).unwrap().0, 333);

        assert_eq!(
            create_order(state, pr(0), token, 250, 1, 0, OrderType::Sell),
            Ok(())
        );

        assert_eq!(
            state.token_balances(pr(0)).get(&token).unwrap().0,
            333 - 250
        );

        assert_eq!(user_orders(state, token, pr(0), OrderType::Buy).count(), 0);
        let sell_orders = user_orders(state, token, pr(0), OrderType::Sell).collect::<Vec<_>>();
        assert_eq!(sell_orders.len(), 1);
        assert_eq!(sell_orders.first().unwrap().amount, 250);

        assert_eq!(
            state.withdraw_liquidity(pr(1), token),
            Err("nothing to withdraw".into())
        );
        assert_eq!(state.withdraw_liquidity(pr(0), token), Ok(333 - 250));

        let sell_orders = user_orders(state, token, pr(0), OrderType::Sell).collect::<Vec<_>>();
        assert_eq!(sell_orders.len(), 1);

        let one_icp = 100000000;
        state
            .add_liquidity(pr(0), PAYMENT_TOKEN_ID, one_icp)
            .unwrap();
        assert_eq!(
            state.withdraw_liquidity(pr(1), PAYMENT_TOKEN_ID),
            Err("nothing to withdraw".into())
        );
        assert_eq!(
            state.withdraw_liquidity(pr(0), PAYMENT_TOKEN_ID),
            Ok((ic_ledger_types::Tokens::from_e8s(one_icp as u64)).e8s() as u128)
        );
    }

    #[test]
    fn test_selling() {
        let state = &mut State::default();
        list_payment_token(state);

        state.revenue_account = Some(pr(255));

        let token = pr(100);

        assert_eq!(
            create_order(state, pr(0), token, 7, 50000000, 0, OrderType::Buy),
            Err("token not listed".into())
        );

        state.add_token(token, "TAGGR".into(), 25, 2, None);

        // buy order for 7 $TAGGR / 0.1 ICP each
        assert_eq!(
            create_order(state, pr(0), token, 7, 10000000, 0, OrderType::Buy),
            Err("no funds available".into())
        );

        state
            .add_liquidity(pr(0), PAYMENT_TOKEN_ID, 8 * 10000000)
            .unwrap();
        assert!(create_order(state, pr(0), token, 7, 10000000, 0, OrderType::Buy).is_ok());

        // buy order for 16 $TAGGR / 0.03 ICP each
        state
            .add_liquidity(pr(1), PAYMENT_TOKEN_ID, 17 * 30000000)
            .unwrap();
        assert!(create_order(state, pr(1), token, 16, 3000000, 0, OrderType::Buy).is_ok());

        // buy order for 25 $TAGGR / 0.01 ICP each
        state
            .add_liquidity(pr(2), PAYMENT_TOKEN_ID, 24 * 1000000)
            .unwrap();
        assert_eq!(
            create_order(state, pr(2), token, 25, 1000000, 0, OrderType::Buy),
            Err("not enough funds available for this order size".into())
        );
        state
            .add_liquidity(pr(2), PAYMENT_TOKEN_ID, 2 * 1000000)
            .unwrap();
        assert!(create_order(state, pr(2), token, 25, 1000000, 0, OrderType::Buy).is_ok());

        // buyer has 0.01 ICP left minus fee
        assert_eq!(state.payment_token_pool().get(&pr(2)).unwrap(), &997500);

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
            trade(
                &mut state.clone(),
                OrderType::Sell,
                seller,
                token,
                5,
                None,
                123456
            ),
            Err("not enough tokens".into())
        );
        state.add_liquidity(seller, token, 250).unwrap();
        assert_eq!(
            trade(state, OrderType::Sell, seller, token, 5, None, 123456),
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
        assert_eq!(state.payment_token_pool().get(&pr(0)).unwrap(), &9993000);
        // fee account has 2 fee
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
            trade(state, OrderType::Sell, seller, token, 10, None, 123457),
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
            trade(state, OrderType::Sell, seller, token, 150, None, 123457),
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
        let fee = trading_fee(v1) + trading_fee(v2) + trading_fee(v3);
        assert_eq!(
            state.payment_token_pool().get(&seller).unwrap(),
            &(v1 + v2 + v3 - fee)
        );
        assert_eq!(
            state.payment_token_pool().get(&pr(255)).unwrap(),
            &(2 * fee)
        );
    }

    #[test]
    fn test_buying() {
        let state = &mut State::default();
        list_payment_token(state);

        state.revenue_account = Some(pr(255));

        let token = pr(100);

        assert_eq!(
            create_order(
                &mut state.clone(),
                pr(0),
                token,
                7,
                5000000,
                0,
                OrderType::Sell
            ),
            Err("token not listed".into())
        );

        state.add_token(token, "TAGGR".into(), 25, 2, None);

        // sell order for 7 $TAGGR / 0.05 ICP each
        assert_eq!(
            create_order(
                &mut state.clone(),
                pr(0),
                token,
                7,
                5000000,
                0,
                OrderType::Sell
            ),
            Err("no funds available".into())
        );

        state.add_liquidity(pr(0), token, 7).unwrap();
        assert!(create_order(state, pr(0), token, 7, 5000000, 0, OrderType::Sell).is_ok());

        // sell order for 16 $TAGGR / 0.03 ICP each
        state.add_liquidity(pr(1), token, 16).unwrap();
        assert!(create_order(state, pr(1), token, 16, 3000000, 0, OrderType::Sell).is_ok());

        // sell order for 25 $TAGGR / 1 ICP each
        state.add_liquidity(pr(2), token, 24).unwrap();
        assert_eq!(
            create_order(
                &mut state.clone(),
                pr(2),
                token,
                25,
                100000000,
                0,
                OrderType::Sell
            ),
            Err("not enough funds available for this order size".into())
        );
        state.add_liquidity(pr(2), token, 1).unwrap();
        assert!(create_order(state, pr(2), token, 25, 100000000, 0, OrderType::Sell).is_ok());

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
            trade(state, OrderType::Buy, buyer, token, 10, None, 123456),
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
            trade(state, OrderType::Buy, buyer, token, 7, None, 123457),
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
            trade(state, OrderType::Buy, buyer, token, 100, None, 123458),
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
        let fee = trading_fee(v1) + trading_fee(v2) + trading_fee(v3);
        assert_eq!(
            state.payment_token_pool().get(&pr(255)).unwrap(),
            &(2 * fee)
        );
    }

    #[test]
    fn test_limit_selling() {
        let state = &mut State::default();
        list_payment_token(state);

        state.revenue_account = Some(pr(255));

        let token = pr(100);

        state.add_token(token, "TAGGR".into(), 25, 2, None);

        // buy order for 7 $TAGGR / 0.1 ICP each
        state
            .add_liquidity(pr(0), PAYMENT_TOKEN_ID, 8 * 10000000)
            .unwrap();
        assert!(create_order(state, pr(0), token, 7, 10000000, 0, OrderType::Buy).is_ok());

        // buy order for 16 $TAGGR / 0.03 ICP each
        state
            .add_liquidity(pr(1), PAYMENT_TOKEN_ID, 17 * 30000000)
            .unwrap();
        assert!(create_order(state, pr(1), token, 16, 3000000, 0, OrderType::Buy).is_ok());

        // buy order for 25 $TAGGR / 0.01 ICP each
        state
            .add_liquidity(pr(2), PAYMENT_TOKEN_ID, 26 * 1000000)
            .unwrap();
        assert!(create_order(state, pr(2), token, 25, 1000000, 0, OrderType::Buy).is_ok());

        // Orer book: 7 @ 0.1, 16 @ 0.03, 25 @ 0.01

        let seller = pr(5);

        state.add_liquidity(seller, token, 250).unwrap();
        assert_eq!(
            trade(
                state,
                OrderType::Sell,
                seller,
                token,
                50,
                Some(2000000),
                123456
            ),
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
        let state = &mut State::default();
        list_payment_token(state);

        state.revenue_account = Some(pr(255));

        let token = pr(100);

        state.add_token(token, "TAGGR".into(), 25, 2, None);

        // sell order for 7 $TAGGR / 0.05 ICP each
        state.add_liquidity(pr(0), token, 7).unwrap();
        assert!(create_order(state, pr(0), token, 7, 5000000, 0, OrderType::Sell).is_ok());

        // sell order for 16 $TAGGR / 0.03 ICP each
        state.add_liquidity(pr(1), token, 16).unwrap();
        assert!(create_order(state, pr(1), token, 16, 3000000, 0, OrderType::Sell).is_ok());

        // sell order for 25 $TAGGR / 1 ICP each
        state.add_liquidity(pr(2), token, 25).unwrap();
        assert!(create_order(state, pr(2), token, 25, 100000000, 0, OrderType::Sell).is_ok());

        // Order book: 16 @ 0.03, 7 @ 0.05, 25 @ 1

        let buyer = pr(5);

        state
            .add_liquidity(buyer, PAYMENT_TOKEN_ID, 12 * 100000000)
            .unwrap();
        assert_eq!(
            trade(
                state,
                OrderType::Buy,
                buyer,
                token,
                50,
                Some(6000000),
                123456
            ),
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
        let state = &mut State::default();
        list_payment_token(state);

        state.revenue_account = Some(pr(255));

        let token = pr(100);

        state.add_token(token, "TAGGR".into(), 25, 2, None);

        // sell order for 7 $TAGGR / 0.05 ICP each
        state.add_liquidity(pr(0), token, 7).unwrap();
        assert!(create_order(state, pr(0), token, 7, 5000000, 0, OrderType::Sell).is_ok());
        assert_eq!(
            create_order(state, pr(0), token, 7, 6000000, 0, OrderType::Sell),
            Err("not enough funds available for this order size".into())
        );
    }
}
