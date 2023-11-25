use candid::CandidType;
use ic_ledger_types::MAINNET_CYCLES_MINTING_CANISTER_ID;
use serde::Deserialize;

#[derive(CandidType, Deserialize)]
struct IcpXdrConversionRate {
    xdr_permyriad_per_icp: u64,
}

#[derive(CandidType, Deserialize)]
struct IcpXdrConversionRateCertifiedResponse {
    data: IcpXdrConversionRate,
}

pub async fn get_xdr_in_e8s() -> Result<u64, String> {
    let (IcpXdrConversionRateCertifiedResponse {
        data: IcpXdrConversionRate {
            xdr_permyriad_per_icp,
        },
    },) = ic_cdk::call(
        MAINNET_CYCLES_MINTING_CANISTER_ID,
        "get_icp_xdr_conversion_rate",
        (),
    )
    .await
    .map_err(|err| format!("couldn't get ICP/XDR ratio: {:?}", err))?;
    Ok((100_000_000.0 / xdr_permyriad_per_icp as f64) as u64 * 10_000)
}
