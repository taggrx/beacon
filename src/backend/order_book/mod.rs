use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque},
};

use candid::{CandidType, Principal};
use serde::{Deserialize, Serialize};

use crate::{icrc1::Value, DAY, HOUR};

pub const PAYMENT_TOKEN_ID: Principal = Principal::from_slice(&[0, 0, 0, 0, 2, 48, 1, 91, 1, 1]);

pub type Timestamp = u64;
pub type Tokens = u128;
pub type TokenId = Principal;
pub type ParticlesPerToken = u128;

pub const TX_FEE: u128 = 20; // 0.XX% per trade side

const ORDER_EXPIRATION_DAYS: u64 = 90;

// This is a cycle drain protection.
const MAX_ORDERS_PER_HOUR: usize = 15;

#[derive(CandidType, Serialize, Deserialize, PartialEq, Eq, Debug, Clone, Copy)]
pub enum OrderType {
    Buy,
    Sell,
}

#[derive(CandidType, Serialize)]
pub enum OrderExecution {
    Filled(u128),
    FilledAndOrderCreated(u128),
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
    // The direction of the order w.r.t to the underlying token.
    // Buy: the user is buying the underlying token for ICP.
    // Sell: the user is selling the underlying token for ICP.
    order_type: OrderType,
    // The user who created the order.
    owner: Principal,
    amount: Tokens,
    price: ParticlesPerToken,
    // The time when the order was created.
    timestamp: Timestamp,
    // Implicit encoding of optional type: 0 means None - not executed yet.
    pub executed: Timestamp,
    // The number of ICRC-1 decimals in the underlying token.
    decimals: u32,
    payment_token_fee: Tokens,
}

impl Order {
    /// The volume of this trade in payment particles.
    pub fn volume(&self) -> Tokens {
        let token_base = 10_u128.pow(self.decimals);
        (self.amount.checked_mul(self.price)).expect("overflow") / token_base
    }

    /// The amount of user's tokens reserved for the trade.
    /// - buy: ICP token + fee.
    /// - sell: the underlying token.
    fn reserved_liquidity(&self) -> Tokens {
        if self.order_type.buy() {
            let volume = self.volume();
            volume + trading_fee(self.payment_token_fee, volume)
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
        assert_eq!(self.order_type, other.order_type);
        assert_eq!(self.executed, other.executed);

        if self.price != other.price {
            return self.price.cmp(&other.price);
        }

        if self.timestamp != other.timestamp {
            return self.timestamp.cmp(&other.timestamp);
        }

        if self.amount != other.amount {
            return self.amount.cmp(&other.amount);
        }

        self.owner.cmp(&other.owner)
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
struct Book {
    // Invariants for x in buyers:
    // - x.order_type == OrderType::Buy
    // - x.executed == 0
    buyers: BTreeSet<Order>,

    // Invariants for x in sellers:
    // - x.order_type == OrderType::Sell
    // - x.executed == 0
    sellers: BTreeSet<Order>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub symbol: String,
    pub fee: Tokens,
    pub decimals: u32,
    pub logo: Option<String>,
    pub timestamp: Timestamp,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct State {
    // All open orders
    orders: BTreeMap<TokenId, Book>,
    // Executed or expired orders.
    pub order_archive: BTreeMap<TokenId, VecDeque<Order>>,
    // How many tokens each user owns.
    pools: BTreeMap<TokenId, BTreeMap<Principal, Tokens>>,
    pub tokens: BTreeMap<TokenId, Metadata>,
    pub revenue_account: Option<Principal>,
    pub logs: VecDeque<(u64, String)>,
    event_id: u64,
    order_activity: HashMap<Principal, HashSet<Timestamp>>,
}

impl State {
    // Count how many orders the user made within hour and
    // throw an error if the number is above `MAX_ORDERS_PER_HOUR`.
    fn record_activity(
        &mut self,
        token: TokenId,
        principal: Principal,
        now: Timestamp,
    ) -> Result<(), String> {
        let metadata = self.tokens.get_mut(&token).ok_or("token not listed")?;
        metadata.timestamp = now;
        match self.order_activity.get_mut(&principal) {
            Some(records) => {
                records.retain(|timestamp| timestamp + HOUR >= now);
                if records.len() >= MAX_ORDERS_PER_HOUR {
                    return Err("too many orders within one hour; please try again later".into());
                }
                records.insert(now);
                Ok(())
            }
            None => {
                self.order_activity.insert(principal, Default::default());
                Ok(())
            }
        }
    }

    /// Closes orders satisfying the given condition.
    ///
    /// The token filter restricts the deletion to the list of tokens if it is not empty.
    ///
    /// To guarantee that this never runs out
    /// of instructions, we need an upper bound on the total number of orders here.
    pub fn close_orders_by_condition(
        &mut self,
        predicate: &dyn Fn(&Order) -> bool,
        token_filter: HashSet<TokenId>,
        max_chunk: usize,
    ) -> usize {
        let mut closed_orders = 0;
        self.orders
            .iter()
            .filter(|(token, _)| token_filter.is_empty() || token_filter.contains(token))
            .flat_map(|(token, book)| {
                book.buyers
                    .iter()
                    .chain(book.sellers.iter())
                    .map(move |order| (*token, order.clone()))
            })
            .filter(|(_, order)| predicate(order))
            .take(max_chunk)
            .collect::<Vec<_>>()
            .into_iter()
            .for_each(
                |(
                    token,
                    Order {
                        order_type,
                        owner,
                        amount,
                        price,
                        timestamp,
                        ..
                    },
                )| {
                    if let Err(err) =
                        self.close_order(owner, token, amount, price, timestamp, order_type)
                    {
                        self.log(format!("failed to close an order: {}", err))
                    } else {
                        closed_orders += 1
                    }
                },
            );

        closed_orders
    }

    pub fn clean_up(&mut self, now: Timestamp) {
        // Rotate logs
        let mut deleted_logs = 0;
        while self.logs.len() > 10000 {
            self.logs.pop_back();
            deleted_logs += 1;
        }

        // Remove all archived orders older than 3 months
        let mut deleted_archived_orders = 0;
        for archive in self.order_archive.values_mut() {
            let length_before = archive.len();
            archive.retain(|order| order.timestamp + 2 * ORDER_EXPIRATION_DAYS * DAY > now);
            deleted_archived_orders += length_before.saturating_sub(archive.len());
        }

        // Close all orders older than 1 months
        let closed_orders = self.close_orders_by_condition(
            &|order| order.timestamp + ORDER_EXPIRATION_DAYS * DAY < now,
            Default::default(),
            100000,
        );

        if closed_orders > 0 || deleted_archived_orders > 0 || deleted_logs > 0 {
            self.log(format!(
                "clean up: {} logs removed, {} archived orders removed, {} expired orders closed",
                deleted_logs, deleted_archived_orders, closed_orders
            ));
        }

        // Delist all inactive tokens.
        //
        // Note that some users still might have funds in the frontend
        // wallet. In this case, the token must be listed again to recover the funds.
        for token_id in self.tokens.keys().copied().collect::<Vec<_>>() {
            if
            // the last order was created more than `2 x ORDER_EXPIRATION_DAYS` ago
            self
                .token(token_id)
                .map(|data| data.timestamp + 2 * ORDER_EXPIRATION_DAYS < now)
                .unwrap_or(true)
                // there are no buy or sell orders
                && self
                    .orders
                    .get(&token_id)
                    .map(|book| book.sellers.is_empty() && book.buyers.is_empty())
                    .unwrap_or(true)
                    // there is no liquidity locked
                    && self.pools.get(&token_id).map(|pool| pool.is_empty()).unwrap_or(true)
            {
                self.tokens.remove(&token_id);
                self.pools.remove(&token_id);
            }
        }
    }

    /// Returns all users that haev open orders.
    pub fn traders(&self) -> usize {
        self.orders
            .values()
            .flat_map(|book| book.sellers.iter().chain(book.buyers.iter()))
            .map(|order| order.owner)
            .collect::<BTreeSet<_>>()
            .len()
    }

    pub fn list_token(
        &mut self,
        token: TokenId,
        metadata: BTreeMap<String, Value>,
        timestamp: Timestamp,
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
            ) => self.add_token(
                token,
                symbol.clone(),
                *fee,
                *decimals as u32,
                match logo {
                    Some(Value::Text(hex)) => Some(hex.clone()),
                    _ => None,
                },
                timestamp,
            ),
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
        price: ParticlesPerToken,
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
        let order = orders
            .get(&Order {
                order_type,
                owner: user,
                price,
                amount,
                timestamp,
                decimals: 0,
                payment_token_fee: 0,
                executed: 0,
            })
            .ok_or("no order found")?
            .clone();
        let reserved_liquidity = order.reserved_liquidity();
        if !orders.remove(&order) {
            return Err("order not found".into());
        }
        self.add_liquidity(
            user,
            if order_type.buy() {
                PAYMENT_TOKEN_ID
            } else {
                token
            },
            reserved_liquidity,
        );
        Ok(())
    }

    /// Returns open orders sorted by "the best price" for the order type.
    /// - Buy: highest price first
    /// - Sell: lowest price first
    /// Note: used in a query and tests only.
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
    /// Note: used in a query and tests only.
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

    /// Transfers the given number of ICP tokens from the user balance to the
    /// revenue account balance.
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

    /// Adds the given tokens to the user account balance.
    pub fn add_liquidity(&mut self, user: Principal, id: TokenId, amount: Tokens) {
        let pool = self.pools.entry(id).or_default();
        let balance = pool.entry(user).or_default();
        *balance += amount;
        self.log(format!(
            "added {} tokens to {} pool for {}",
            amount, id, user,
        ));
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
        timestamp: Timestamp,
    ) -> Result<(), String> {
        if let Some(current_meta) = self.tokens.get(&id) {
            // If this is a relisting and the fee or the decimals have changed, close all orders first.
            if current_meta.fee != fee || current_meta.decimals != decimals {
                self.close_orders_by_condition(&|_| true, [id].iter().copied().collect(), 100000);
                if let Some(order_book) = self.orders.get(&id) {
                    if !order_book.buyers.is_empty() || !order_book.sellers.is_empty() {
                        return Err("couldn't close all orders".into());
                    }
                }
            }
        }
        self.tokens.insert(
            id,
            Metadata {
                symbol,
                logo,
                fee,
                decimals,
                timestamp,
            },
        );
        if let std::collections::btree_map::Entry::Vacant(e) = self.pools.entry(id) {
            e.insert(Default::default());
            self.log(format!("token {} was listed", id));
        } else {
            self.log(format!("token {} was re-listed", id));
        }
        Ok(())
    }

    pub fn create_order(
        &mut self,
        user: Principal,
        token: TokenId,
        amount: Tokens,
        price: ParticlesPerToken,
        timestamp: Timestamp,
        order_type: OrderType,
    ) -> Result<(), String> {
        if price == 0 {
            return Err("limit price is 0".into());
        }

        self.record_activity(token, user, timestamp)?;

        assert_ne!(
            token, PAYMENT_TOKEN_ID,
            "no orders for payment tokens are possible"
        );

        let metadata = self.tokens.get(&token).ok_or("token not listed")?;
        let payment_token_fee = self
            .tokens
            .get(&PAYMENT_TOKEN_ID)
            .ok_or("payment token not listed")?
            .fee;
        assert_ne!(
            token, PAYMENT_TOKEN_ID,
            "no orders for payment tokens are possible"
        );

        let order = Order {
            order_type,
            owner: user,
            amount,
            price,
            decimals: metadata.decimals,
            payment_token_fee,
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

        let volume = order.volume();
        let fee = trading_fee(order.payment_token_fee, volume);
        if fee * 10 > volume {
            return Err("the order is too small".into());
        }

        let inserted = if order_type.buy() {
            order_book.buyers.insert(order)
        } else {
            order_book.sellers.insert(order)
        };
        if !inserted {
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
        price: ParticlesPerToken,
        now: Timestamp,
    ) -> Result<OrderExecution, String> {
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
        if filled < amount && price > 0 {
            self.create_order(
                user,
                token,
                amount.saturating_sub(filled),
                price,
                now,
                trade_type,
            )?;
            Ok(OrderExecution::FilledAndOrderCreated(filled))
        } else {
            Ok(OrderExecution::Filled(filled))
        }
    }

    fn execute_trade(
        &mut self,
        trade_type: OrderType,
        trader: Principal,
        token: TokenId,
        mut amount: u128,
        limit: Option<ParticlesPerToken>,
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
            // if limit was set and we discover the first order with the price not matching the
            // limit, stop filling orders
            if let Some(limit) = limit {
                if trade_type.buy() && limit < order.price
                    || trade_type.sell() && limit > order.price
                {
                    orders.insert(order);
                    break;
                }
            }

            amount = if order.amount > amount {
                let prev_reserved_liquidity = order.reserved_liquidity();
                // partial order fill - create a new one for left overs
                let mut remaining_order = order.clone();
                remaining_order.amount = order.amount - amount;

                let volume = remaining_order.volume();
                let fee = trading_fee(remaining_order.payment_token_fee, volume);
                assert!(volume > fee, "dust orders are not supported");

                let new_reserved_liquidity = remaining_order.reserved_liquidity();

                assert!(orders.insert(remaining_order), "order overwritten");
                order.amount = amount;
                let freed_liquidity = prev_reserved_liquidity
                    .checked_sub(new_reserved_liquidity + order.reserved_liquidity())
                    .expect("underflow");
                if freed_liquidity > 0 {
                    // Freeing of liquidity on an order split can only happen for sell orders,
                    // because the reserved ICP liquidity is computed using integer division.
                    assert!(trade_type.sell());
                    assert!(order.order_type.buy());
                    if let Some(liquidity) = self
                        .pools
                        .get_mut(&PAYMENT_TOKEN_ID)
                        .and_then(|pool| pool.get_mut(&order.owner))
                    {
                        *liquidity += freed_liquidity;
                    }
                }
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
                    checked_sum(Box::new(pool.values().copied()))
                        + if id == &PAYMENT_TOKEN_ID {
                            checked_sum(Box::new(self.orders.values().flat_map(|book| {
                                book.buyers.iter().map(|order| order.reserved_liquidity())
                            })))
                        } else {
                            self.orders
                                .get(id)
                                .map(|book| {
                                    checked_sum(Box::new(
                                        book.sellers.iter().map(|order| order.reserved_liquidity()),
                                    ))
                                })
                                .unwrap_or_default()
                        },
                )
            })
            .collect()
    }

    #[cfg(feature = "dev")]
    // This method is used for local testing only.
    pub fn replace_user_id(&mut self, old: Principal, new: Principal) {
        self.orders.values_mut().for_each(|book| {
            let mod_orders = book
                .buyers
                .clone()
                .into_iter()
                .map(|mut order| {
                    if order.owner == old {
                        order.owner = new;
                    }
                    order
                })
                .collect();
            book.buyers = mod_orders;
            let mod_orders = book
                .sellers
                .clone()
                .into_iter()
                .map(|mut order| {
                    if order.owner == old {
                        order.owner = new;
                    }
                    order
                })
                .collect();
            book.sellers = mod_orders;
        });
        for pool in self.pools.values_mut() {
            if let Some(balance) = pool.remove(&old) {
                pool.insert(new, balance);
            }
        }
    }

    #[cfg(feature = "dev")]
    // This method is used for local testing only.
    pub fn replace_canister_id(&mut self, old: Principal, new: Principal) {
        if let Some(orders) = self.orders.remove(&old) {
            self.orders.insert(new, orders);
        }
        if let Some(pool) = self.pools.remove(&old) {
            self.pools.insert(new, pool);
        }
        if let Some(metadata) = self.tokens.remove(&old) {
            self.tokens.insert(new, metadata);
        }
        if let Some(archive) = self.order_archive.remove(&old) {
            self.order_archive.insert(new, archive);
        }
    }
}

fn checked_sum(iter: Box<dyn Iterator<Item = Tokens> + '_>) -> Tokens {
    let mut result: Tokens = 0;
    for value in iter {
        result = result.checked_add(value).expect("overflow");
    }
    result
}

/// Updates balances to execute the given order.
/// The trader's balances are in the pool.
/// The order owner's balances are partially in the pool and the order itself.
/// 1) Buy case:
/// - the trader buys N tokens for M + FEE ICP.
/// - the order contains N tokens.
/// - the type of the order is sell.
/// - pool[ICP][trader] -= M + FEE
/// - pool[ICP][order.owner] += M - FEE
/// - pool[token][trader] += order.amount
/// - pool[ICP][revenue] += 2*FEE
///
/// 2) Sell case:
/// - the trader sell N tokens for M + FEE ICP.
/// - the order contains M + FEE ICP.
/// - the type of the order is buy.
/// - pool[ICP][trader] += M - FEE
/// - pool[token][order.owner] += order.amount
/// - pool[token][trader] -= order.amount
/// - pool[ICP][revenue] += 2*FEE
fn adjust_pools(
    pools: &mut BTreeMap<TokenId, BTreeMap<Principal, Tokens>>,
    trader: Principal,
    token: TokenId,
    order: &Order,
    revenue_account: Principal,
    trade_type: OrderType,
) -> Result<(), String> {
    // since the liquidity is locked inside the order,
    // we need to know where we should avoid adjusting pools
    assert_ne!(order.order_type, trade_type);

    let (payment_receiver, token_receiver) = if trade_type.buy() {
        (order.owner, trader)
    } else {
        (trader, order.owner)
    };

    let token_pool = pools.get_mut(&token).ok_or("no token pool found")?;

    // We only need to subtract token liquidity if we're executing a selling trade, because
    // the liquidity for the buy order has already been reserved at order creation.
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

    let volume = order.volume();
    let fee = trading_fee(order.payment_token_fee, volume);

    // We only need to subtract payment liquidity if we're executing a buying trade, because
    // the liquidity for the sell order has already been reserved at order creation.
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

fn trading_fee(fee: Tokens, volume: Tokens) -> Tokens {
    (volume * TX_FEE / fee).max(1)
}

#[cfg(test)]
mod tests {

    use crate::{mutate, read, unsafe_mutate};

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
        price: ParticlesPerToken,
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
        price: ParticlesPerToken,
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
        limit: Option<ParticlesPerToken>,
        time: Timestamp,
    ) -> Result<u128, String> {
        let fum = state.funds_under_management();
        let result = state.execute_trade(trade_type, trader, token, amount, limit, time);
        if result.is_ok() {
            assert_eq!(fum, state.funds_under_management());
        }
        result
    }

    fn list_test_token(state: &mut State, token: TokenId, decimals: u32) {
        state
            .add_token(
                token,
                "TAGGR".into(),
                25, // fee
                decimals,
                None,
                0,
            )
            .unwrap();
    }

    fn list_payment_token(state: &mut State) {
        state.revenue_account = Some(pr(255));
        state.pools.insert(PAYMENT_TOKEN_ID, Default::default());
        state.tokens.insert(
            PAYMENT_TOKEN_ID,
            Metadata {
                symbol: "USD".into(),
                fee: 10000,
                decimals: 8,
                logo: None,
                timestamp: 0,
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
            decimals: 6,
            timestamp: 111,
            executed: 0,
            payment_token_fee: 10000,
        };
        let mut o2 = Order {
            order_type: OrderType::Buy,
            owner: pr(16),
            amount: 32,
            price: 0,
            decimals: 6,
            timestamp: 111,
            executed: 0,
            payment_token_fee: 10000,
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
        let token = pr(100);
        list_payment_token(state);
        list_test_token(state, token, 2);

        state.add_liquidity(pr(1), PAYMENT_TOKEN_ID, 210);
        assert_eq!(trading_fee(10000, 20000), 40);
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

        state.add_liquidity(pr(0), token, 1);
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

        list_test_token(state, token, 2);

        state.add_liquidity(pr(0), token, 111);
        state.add_liquidity(pr(0), token, 222);

        assert_eq!(state.token_balances(pr(0)).get(&token).unwrap().0, 333);

        assert_eq!(
            create_order(state, pr(0), token, 250, 10, 0, OrderType::Sell),
            Ok(())
        );
        assert_eq!(
            create_order(state, pr(0), token, 50, 30, 0, OrderType::Sell),
            Ok(())
        );

        assert_eq!(
            state.token_balances(pr(0)).get(&token).unwrap().0,
            333 - 250 - 50
        );
        assert_eq!(
            close_order(state, pr(0), token, 50, 30, 0, OrderType::Sell),
            Ok(())
        );
        assert_eq!(
            close_order(state, pr(0), token, 250, 10, 0, OrderType::Sell),
            Ok(())
        );
        assert_eq!(state.token_balances(pr(0)).get(&token).unwrap().0, 333);

        state.add_liquidity(pr(0), PAYMENT_TOKEN_ID, 8 * 100000);
        assert!(create_order(state, pr(0), token, 3, 10000000, 0, OrderType::Buy).is_ok());

        let volume = 3 * 100000;

        assert_eq!(
            state
                .token_balances(pr(0))
                .get(&PAYMENT_TOKEN_ID)
                .copied()
                .unwrap()
                .0,
            8 * 100000 - volume - trading_fee(10000, volume)
        );

        assert_eq!(
            create_order(state, pr(0), token, 3, 10000000, 0, OrderType::Buy),
            Err("order exists already".into())
        );

        assert!(create_order(state, pr(0), token, 4, 10000000, 0, OrderType::Buy).is_ok());

        let volume2 = 4 * 100000;
        assert_eq!(
            state
                .token_balances(pr(0))
                .get(&PAYMENT_TOKEN_ID)
                .copied()
                .unwrap()
                .0,
            8 * 100000
                - volume
                - trading_fee(10000, volume)
                - volume2
                - trading_fee(10000, volume2)
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
            8 * 100000
        );
    }

    #[test]
    fn test_liquidity_adding_and_withdrawals() {
        let state = &mut State::default();
        list_payment_token(state);

        let token = pr(100);

        list_test_token(state, token, 2);

        state.add_liquidity(pr(0), token, 111);
        state.add_liquidity(pr(0), token, 222);

        assert_eq!(state.token_balances(pr(0)).get(&token).unwrap().0, 333);

        assert_eq!(
            create_order(state, pr(0), token, 250, 10, 0, OrderType::Sell),
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
        state.add_liquidity(pr(0), PAYMENT_TOKEN_ID, one_icp);
        assert_eq!(
            state.withdraw_liquidity(pr(1), PAYMENT_TOKEN_ID),
            Err("nothing to withdraw".into())
        );
        assert_eq!(
            state.withdraw_liquidity(pr(0), PAYMENT_TOKEN_ID),
            Ok(100000000)
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

        list_test_token(state, token, 2);

        // buy order for 7 $TAGGR / 0.1 ICP each
        assert_eq!(
            create_order(state, pr(0), token, 7, 10000000, 0, OrderType::Buy),
            Err("no funds available".into())
        );

        state.add_liquidity(pr(0), PAYMENT_TOKEN_ID, 8 * 100000);
        assert!(create_order(state, pr(0), token, 7, 10000000, 0, OrderType::Buy).is_ok());

        // buy order for 16 $TAGGR / 0.03 ICP each
        state.add_liquidity(pr(1), PAYMENT_TOKEN_ID, 17 * 300000);
        assert!(create_order(state, pr(1), token, 16, 3000000, 0, OrderType::Buy).is_ok());

        // buy order for 25 $TAGGR / 0.01 ICP each
        state.add_liquidity(pr(2), PAYMENT_TOKEN_ID, 24 * 10000);
        assert_eq!(
            create_order(state, pr(2), token, 25, 1000000, 0, OrderType::Buy),
            Err("not enough funds available for this order size".into())
        );
        state.add_liquidity(pr(2), PAYMENT_TOKEN_ID, 2 * 10000);
        assert!(create_order(state, pr(2), token, 25, 1000000, 0, OrderType::Buy).is_ok());

        // buyer has 0.01 ICP left minus fee
        assert_eq!(state.payment_token_pool().get(&pr(2)).unwrap(), &9500);

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
        state.add_liquidity(seller, token, 250);
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
        let volume = 500000;
        let fee_per_side = trading_fee(10000, volume);
        assert_eq!(
            state.payment_token_pool().get(&seller).unwrap(),
            &(volume - fee_per_side)
        );
        // buyer should have previous amount - volume - fee;
        assert_eq!(state.payment_token_pool().get(&pr(0)).unwrap(), &98600);
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
        let (v1, v2, v3) = (25 * 10000, 16 * 30000, 7 * 100000);
        let fee = trading_fee(10000, v1) + trading_fee(10000, v2) + trading_fee(10000, v3);
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

        list_test_token(state, token, 2);

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

        state.add_liquidity(pr(0), token, 7);
        assert!(create_order(state, pr(0), token, 7, 5000000, 0, OrderType::Sell).is_ok());

        // sell order for 16 $TAGGR / 0.03 ICP each
        state.add_liquidity(pr(1), token, 16);
        assert!(create_order(state, pr(1), token, 16, 3000000, 0, OrderType::Sell).is_ok());

        // sell order for 25 $TAGGR / 1 ICP each
        state.add_liquidity(pr(2), token, 24);
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
        state.add_liquidity(pr(2), token, 1);
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
        state.add_liquidity(buyer, PAYMENT_TOKEN_ID, 12 * 30000);
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
        state.add_liquidity(buyer, PAYMENT_TOKEN_ID, 6 * 30000 + 2 * 5000);
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

        state.add_liquidity(buyer, PAYMENT_TOKEN_ID, 6 * 50000 + 28 * 1000000);

        assert_eq!(
            trade(state, OrderType::Buy, buyer, token, 100, None, 123458),
            Ok(31)
        );

        // all sellers got ICP
        let (v2, v1, v3) = (16 * 30000, 7 * 50000, 25 * 1000000);
        assert_eq!(
            state.payment_token_pool().get(&pr(0)).unwrap(),
            &(v1 - trading_fee(10000, v1))
        );
        assert_eq!(
            state.payment_token_pool().get(&pr(1)).unwrap(),
            &(v2 - trading_fee(10000, v2))
        );
        assert_eq!(
            state.payment_token_pool().get(&pr(2)).unwrap(),
            &(v3 - trading_fee(10000, v3))
        );

        // executed orders: 16 @ 0.03, 7 @ 0.05, 25 @ 1
        let fee = trading_fee(10000, v1) + trading_fee(10000, v2) + trading_fee(10000, v3);
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

        list_test_token(state, token, 2);

        // buy order for 7 $TAGGR / 0.1 ICP each
        state.add_liquidity(pr(0), PAYMENT_TOKEN_ID, 8 * 10000000);
        assert!(create_order(state, pr(0), token, 7, 10000000, 0, OrderType::Buy).is_ok());

        // buy order for 16 $TAGGR / 0.03 ICP each
        state.add_liquidity(pr(1), PAYMENT_TOKEN_ID, 17 * 30000000);
        assert!(create_order(state, pr(1), token, 16, 3000000, 0, OrderType::Buy).is_ok());

        // buy order for 25 $TAGGR / 0.01 ICP each
        state.add_liquidity(pr(2), PAYMENT_TOKEN_ID, 26 * 1000000);
        assert!(create_order(state, pr(2), token, 25, 1000000, 0, OrderType::Buy).is_ok());

        // Orer book: 7 @ 0.1, 16 @ 0.03, 25 @ 0.01

        let seller = pr(5);

        state.add_liquidity(seller, token, 250);
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

        list_test_token(state, token, 2);

        // sell order for 7 $TAGGR / 0.05 ICP each
        state.add_liquidity(pr(0), token, 7);
        assert!(create_order(state, pr(0), token, 7, 5000000, 0, OrderType::Sell).is_ok());

        // sell order for 16 $TAGGR / 0.03 ICP each
        state.add_liquidity(pr(1), token, 16);
        assert!(create_order(state, pr(1), token, 16, 3000000, 0, OrderType::Sell).is_ok());

        // sell order for 25 $TAGGR / 1 ICP each
        state.add_liquidity(pr(2), token, 25);
        assert!(create_order(state, pr(2), token, 25, 100000000, 0, OrderType::Sell).is_ok());

        // Order book: 16 @ 0.03, 7 @ 0.05, 25 @ 1

        let buyer = pr(5);

        state.add_liquidity(buyer, PAYMENT_TOKEN_ID, 12 * 1000000);
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
        let (v2, v1) = (16 * 30000, 7 * 50000);
        assert_eq!(
            state.payment_token_pool().get(&pr(0)).unwrap(),
            &(v1 - trading_fee(10000, v1))
        );
        assert_eq!(
            state.payment_token_pool().get(&pr(1)).unwrap(),
            &(v2 - trading_fee(10000, v2))
        );
        assert_eq!(state.payment_token_pool().get(&pr(2)), None);
    }

    #[test]
    fn test_liquitidy_lock() {
        let state = &mut State::default();
        list_payment_token(state);

        state.revenue_account = Some(pr(255));

        let token = pr(100);

        list_test_token(state, token, 2);

        // sell order for 7 $TAGGR / 0.05 ICP each
        state.add_liquidity(pr(0), token, 7);
        assert!(create_order(state, pr(0), token, 7, 5000000, 0, OrderType::Sell).is_ok());
        assert_eq!(
            create_order(state, pr(0), token, 7, 6000000, 0, OrderType::Sell),
            Err("not enough funds available for this order size".into())
        );
    }

    #[test]
    fn test_partial_order_liquidity_preservation() {
        let seller = pr(5);
        let token = pr(100);
        unsafe_mutate(|state| {
            list_payment_token(state);

            state.revenue_account = Some(pr(255));

            list_test_token(state, token, 2);

            state.add_liquidity(pr(0), PAYMENT_TOKEN_ID, 6 * 9500000);
            assert!(create_order(state, pr(0), token, 5, 9500000, 0, OrderType::Buy).is_ok());

            state.add_liquidity(pr(1), PAYMENT_TOKEN_ID, 601 * 9500000);
            assert!(create_order(state, pr(1), token, 600, 9500000, 0, OrderType::Buy).is_ok());

            state.add_liquidity(seller, token, 10);
        });
        assert_eq!(read(|state| state.pools.len()), 2);
        assert_eq!(
            mutate(|state| trade(state, OrderType::Sell, seller, token, 5, None, 123456)),
            Ok(5)
        );
    }
}
