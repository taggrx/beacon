use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
};

use candid::Principal;
use serde::{Deserialize, Serialize};

pub type Tokens = u128;
pub type TokenId = Principal;

#[derive(Serialize, Deserialize, PartialEq, Eq)]
struct Order {
    owner: Principal,
    amount: Tokens,
}

impl PartialOrd for Order {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.amount.cmp(&other.amount))
    }
}

impl Ord for Order {
    fn cmp(&self, other: &Self) -> Ordering {
        self.amount.cmp(&other.amount)
    }
}

#[derive(Serialize, Deserialize)]
struct Book {
    buyers: BTreeSet<Order>,
    sellers: BTreeSet<Order>,
}

type Timestamp = u64;
type PriceDelta = i64;

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
    price_moves: BTreeMap<TokenId, Vec<(Timestamp, PriceDelta)>>,
    pools: BTreeMap<TokenId, (Principal, Tokens)>,
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

    pub async fn create_order(
        &mut self,
        caller: Principal,
        id: TokenId,
        amount: Tokens,
    ) -> Result<(), String> {
        panic!()
    }
}
