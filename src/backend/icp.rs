use candid::Principal;
use ic_cdk::api::id;
use ic_ledger_types::{
    AccountBalanceArgs, AccountIdentifier, BlockIndex, Memo, Subaccount, Tokens, TransferArgs,
    TransferResult, DEFAULT_FEE, DEFAULT_SUBACCOUNT, MAINNET_LEDGER_CANISTER_ID,
};

use crate::read;

pub fn revenue_account() -> AccountIdentifier {
    AccountIdentifier::new(
        &read(|state| state.revenue_account).expect("no revenue subaccount set"),
        &DEFAULT_SUBACCOUNT,
    )
}
pub fn user_account(principal: Principal) -> AccountIdentifier {
    AccountIdentifier::new(&id(), &principal_to_subaccount(&principal))
}

pub fn main_account() -> AccountIdentifier {
    AccountIdentifier::new(&id(), &DEFAULT_SUBACCOUNT)
}

pub async fn transfer(
    to: AccountIdentifier,
    amount: Tokens,
    memo: Memo,
    sub_account: Option<Subaccount>,
) -> Result<BlockIndex, String> {
    if amount < DEFAULT_FEE {
        return Err("can't transfer amounts smaller than the fee".into());
    }
    let (result,): (TransferResult,) = ic_cdk::call(
        MAINNET_LEDGER_CANISTER_ID,
        "transfer",
        (TransferArgs {
            created_at_time: None,
            memo,
            amount: amount - DEFAULT_FEE,
            fee: DEFAULT_FEE,
            to,
            from_subaccount: sub_account,
        },),
    )
    .await
    .map_err(|err| format!("call to ledger failed: {:?}", err))?;
    result.map_err(|err| {
        format!(
            "transfer of `{}` ICP from `{}` failed: {:?}",
            amount,
            AccountIdentifier::new(&id(), &sub_account.unwrap_or(DEFAULT_SUBACCOUNT)),
            err
        )
    })
}

pub async fn account_balance_of_principal(principal: Principal) -> Tokens {
    account_balance(AccountIdentifier::new(
        &id(),
        &principal_to_subaccount(&principal),
    ))
    .await
}

async fn account_balance(account: AccountIdentifier) -> Tokens {
    let (balance,): (Tokens,) = ic_cdk::call(
        MAINNET_LEDGER_CANISTER_ID,
        "account_balance",
        (AccountBalanceArgs { account },),
    )
    .await
    .expect("couldn't check balance");
    balance
}

pub fn principal_to_subaccount(principal_id: &Principal) -> Subaccount {
    let mut subaccount = [0; std::mem::size_of::<Subaccount>()];
    let principal_id = principal_id.as_slice();
    subaccount[0] = principal_id.len() as u8;
    subaccount[1..1 + principal_id.len()].copy_from_slice(principal_id);
    Subaccount(subaccount)
}
