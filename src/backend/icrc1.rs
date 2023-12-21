use ic_cdk::id;
use std::collections::BTreeMap;

use candid::{CandidType, Deserialize, Principal};
use serde::Serialize;

use crate::order_book::{TokenId, Tokens};

type Timestamp = u64;

pub type Subaccount = Vec<u8>;

type Memo = [u8; 32];

#[derive(CandidType, Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub struct Account {
    pub owner: Principal,
    pub subaccount: Option<Subaccount>,
}

#[derive(CandidType, Serialize, Deserialize)]
pub struct TransferArgs {
    from_subaccount: Option<Subaccount>,
    to: Account,
    amount: u128,
    fee: Option<u128>,
    memo: Option<Memo>,
    created_at_time: Option<Timestamp>,
}

#[derive(CandidType, Debug, PartialEq, Deserialize, Serialize)]
pub struct InsufficientFunds {
    balance: u128,
}

#[derive(CandidType, Debug, PartialEq, Deserialize, Serialize)]
pub struct CreatedInFuture {
    ledger_time: Timestamp,
}

#[derive(CandidType, Debug, PartialEq, Deserialize, Serialize)]
pub struct GenericError {
    error_code: u128,
    message: String,
}

#[derive(CandidType, Debug, PartialEq, Deserialize, Serialize)]
pub struct BadFee {
    expected_fee: u128,
}

#[derive(CandidType, Debug, PartialEq, Deserialize, Serialize)]
pub enum TransferError {
    BadFee(BadFee),
    // BadBurn(BadBurn),
    // Duplicate(Duplicate),
    TemporarilyUnavailable,
    InsufficientFunds(InsufficientFunds),
    TooOld,
    CreatedInFuture(CreatedInFuture),
    GenericError(GenericError),
}

#[derive(Debug, CandidType, Deserialize)]
pub enum Value {
    Nat(u128),
    Text(String),
    // Int(i64),
    // Blob(Vec<u8>),
}

pub async fn balance_of(token: TokenId, account: &Account) -> Result<Tokens, String> {
    let (result,): (Tokens,) = ic_cdk::call(token, "icrc1_balance_of", (&account,))
        .await
        .map_err(|err| format!("call failed: {:?}", err))?;
    Ok(result)
}

pub async fn metadata(token: TokenId) -> Result<BTreeMap<String, Value>, String> {
    let (result,): (Vec<(String, Value)>,) = ic_cdk::call(token, "icrc1_metadata", ((),))
        .await
        .map_err(|err| format!("call failed: {:?}", err))?;
    let mut data = result.into_iter().collect::<BTreeMap<_, _>>();

    if !data.contains_key("icrc1:symbol") {
        let (symbol,): (String,) = ic_cdk::call(token, "icrc1_symbol", ((),))
            .await
            .map_err(|err| format!("call failed: {:?}", err))?;
        data.insert("icrc1:symbol".to_string(), Value::Text(symbol));
    }

    if !data.contains_key("icrc1:fee") {
        let (fee,): (u128,) = ic_cdk::call(token, "icrc1_fee", ((),))
            .await
            .map_err(|err| format!("call failed: {:?}", err))?;
        data.insert("icrc1:fee".to_string(), Value::Nat(fee));
    }

    if !data.contains_key("icrc1:decimals") {
        let (decimals,): (u8,) = ic_cdk::call(token, "icrc1_decimals", ((),))
            .await
            .map_err(|err| format!("call failed: {:?}", err))?;
        data.insert("icrc1:decimals".to_string(), Value::Nat(decimals as u128));
    }

    Ok(data)
}

pub async fn transfer(
    token: TokenId,
    from_subaccount: Option<Subaccount>,
    to: Account,
    amount: Tokens,
) -> Result<u128, String> {
    let args = TransferArgs {
        from_subaccount,
        to,
        amount,
        memo: None,
        fee: None,
        created_at_time: None,
    };
    let (result,): (Result<u128, TransferError>,) = ic_cdk::call(token, "icrc1_transfer", (args,))
        .await
        .map_err(|err| format!("call failed: {:?}", err))?;
    result.map_err(|err| format!("{:?}", err))
}

pub fn main_account() -> Account {
    Account {
        owner: id(),
        subaccount: None,
    }
}

pub fn user_account(user: Principal) -> Account {
    let mut subaccount = user.as_slice().to_vec();
    subaccount.resize(32, 0);
    Account {
        owner: id(),
        subaccount: Some(subaccount),
    }
}
