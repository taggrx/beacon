use data_encoding::Specification;
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

pub fn crc32(data: &[u8]) -> u32 {
    use crc32fast::Hasher;
    let mut hasher = Hasher::new();
    hasher.update(data);
    hasher.finalize()
}

fn base32_lowercase_no_padding(data: &[u8]) -> String {
    let mut spec = Specification::new();
    spec.symbols.push_str("abcdefghijklmnopqrstuvwxyz234567");
    spec.padding = None;
    let encoding = spec.encoding().unwrap();
    encoding.encode(data)
}

impl Account {
    pub fn to_string(&self) -> String {
        let mut bytes = self.owner.as_slice().to_vec();
        if let Some(sub) = self.subaccount.as_ref() {
            bytes.extend_from_slice(sub);
            let checksum = crc32(&bytes).to_be_bytes();
            format!(
                "{}-{}.{}",
                self.owner.to_string(),
                base32_lowercase_no_padding(checksum.as_slice()),
                hex::encode(sub).trim_start_matches('0').to_string()
            )
        } else {
            self.owner.to_text()
        }
    }

    #[allow(dead_code)]
    pub fn from_string(value: &str) -> Result<Self, String> {
        match value.split(".").collect::<Vec<_>>().as_slice() {
            [owner] => Ok(Account {
                owner: Principal::from_text(owner).map_err(|err| err.to_string())?,
                subaccount: None,
            }),
            [owner_checksum, hex] => {
                let mut parts = owner_checksum.split("-").collect::<Vec<_>>();
                let checksum_str = parts.pop().ok_or("couldn't parse the account")?;
                let owner = Principal::from_text(parts.join("-")).map_err(|err| err.to_string())?;
                let mut bytes = owner.as_slice().to_vec();
                let subaccount =
                    hex::decode(format!("{:0>64}", hex)).map_err(|err| err.to_string())?;
                bytes.extend_from_slice(&subaccount);
                let checksum = crc32(&bytes).to_be_bytes();
                if checksum_str != base32_lowercase_no_padding(checksum.as_slice()).as_str() {
                    return Err("couldn't parse the account".into());
                }
                Ok(Account {
                    owner,
                    subaccount: Some(subaccount),
                })
            }
            _ => Err("couldn't parse the account".into()),
        }
    }
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

pub async fn balance_of(token: TokenId, account: Account) -> Result<Tokens, String> {
    let (result,): (Tokens,) = ic_cdk::call(token, "icrc1_balance_of", (account,))
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

pub async fn transfer(token: TokenId, args: TransferArgs) -> Result<u128, String> {
    let (result,): (Result<u128, TransferError>,) = ic_cdk::call(token, "icrc1_transfer", (args,))
        .await
        .map_err(|err| format!("call failed: {:?}", err))?;
    result.map_err(|err| format!("{:?}", err))
}

pub fn account(owner: Principal, user: Principal) -> Account {
    let mut subaccount = user.as_slice().to_vec();
    subaccount.resize(32, 0);
    Account {
        owner,
        subaccount: Some(subaccount),
    }
}

#[cfg(test)]
mod tests {
    use candid::Principal;

    use super::Account;

    #[test]
    fn test_account_encoding() {
        let acc = Account {
            owner: Principal::from_text(
                "k2t6j-2nvnp-4zjm3-25dtz-6xhaa-c7boj-5gayf-oj3xs-i43lp-teztq-6ae",
            )
            .unwrap(),
            subaccount: None,
        };

        let encoded = "k2t6j-2nvnp-4zjm3-25dtz-6xhaa-c7boj-5gayf-oj3xs-i43lp-teztq-6ae";
        assert_eq!(&acc.to_string(), encoded);
        assert_eq!(acc, Account::from_string(encoded).unwrap());

        let acc = Account {
            owner: Principal::from_text(
                "k2t6j-2nvnp-4zjm3-25dtz-6xhaa-c7boj-5gayf-oj3xs-i43lp-teztq-6ae",
            )
            .unwrap(),
            subaccount: Some(vec![
                1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
                24, 25, 26, 27, 28, 29, 30, 31, 32,
            ]),
        };

        let encoded =
            "k2t6j-2nvnp-4zjm3-25dtz-6xhaa-c7boj-5gayf-oj3xs-i43lp-teztq-6ae-dfxgiyy.102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20";
        assert_eq!(&acc.to_string(), encoded);
        assert_eq!(acc, Account::from_string(encoded).unwrap());

        let acc = Account {
            owner: Principal::from_text(
                "k2t6j-2nvnp-4zjm3-25dtz-6xhaa-c7boj-5gayf-oj3xs-i43lp-teztq-6ae",
            )
            .unwrap(),
            subaccount: Some(vec![
                0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 1,
            ]),
        };

        let encoded = "k2t6j-2nvnp-4zjm3-25dtz-6xhaa-c7boj-5gayf-oj3xs-i43lp-teztq-6ae-6cc627i.1";
        assert_eq!(&acc.to_string(), encoded);
        assert_eq!(acc, Account::from_string(encoded).unwrap());

        let acc = Account {
            owner: Principal::from_text(
                "k2t6j-2nvnp-4zjm3-25dtz-6xhaa-c7boj-5gayf-oj3xs-i43lp-teztq-6ae",
            )
            .unwrap(),
            subaccount: Some(vec![
                0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
                0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c,
                0x1d, 0x1e, 0x1f, 0x20,
            ]),
        };

        let encoded =
            "k2t6j-2nvnp-4zjm3-25dtz-6xhaa-c7boj-5gayf-oj3xs-i43lp-teztq-6ae-dfxgiyy.102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20";
        assert_eq!(&acc.to_string(), encoded);
        assert_eq!(acc, Account::from_string(encoded).unwrap());
    }
}
