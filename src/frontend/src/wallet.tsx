import * as React from "react";
import { Button, CopyToClipboard, token } from "./common";
import { Principal } from "@dfinity/principal";

export const Wallet = ({}) => {
    const internalRenderedBalances = renderBalances(
        window.internalBalances,
        "internal",
    );
    const renderedBalances = renderBalances(window.balances);
    return (
        <div id="wallet" className="modal column_container">
            {renderedBalances.length > 0 && (
                <>
                    <h3>FUNDS IN WALLET</h3>
                    {renderedBalances}
                </>
            )}
            {internalRenderedBalances.length > 0 && (
                <>
                    <h3>FUNDS ON BEACON</h3>
                    {internalRenderedBalances}
                </>
            )}
            <h3>PRINCIPAL</h3>
            <div
                style={{ fontSize: "small" }}
                className="row_container vcentered"
            >
                <CopyToClipboard
                    classNameArg="max_width_col"
                    value={window.principalId.toString()}
                />
                <Button label="REFRESH" onClick={window.refreshBackendData} />
                <Button
                    label="LOGOUT"
                    onClick={async () => {
                        await window.authClient.logout();
                        location.reload();
                    }}
                />
            </div>
        </div>
    );
};

const renderBalances = (
    balances: { [key: string]: bigint },
    internal?: string,
) =>
    Object.entries(window.tokenData)
        .filter(([id]) => id in balances && balances[id] > 0)
        .map(([id, data]) => (
            <BalanceLine
                key={id}
                id={id}
                logo={data.logo}
                symbol={data.symbol}
                balance={balances[id]}
                decimals={data.decimals}
                fee={data.fee}
                internal={!!internal}
            />
        ));

const BalanceLine = ({
    id,
    logo,
    symbol,
    balance,
    decimals,
    fee,
    internal,
}: {
    id: string;
    logo: string;
    symbol: string;
    balance: bigint;
    decimals: number;
    fee: bigint;
    internal: boolean;
}) => {
    const [status, setStatus] = React.useState("");
    const showStatus = (msg: string) => {
        setStatus(msg);
        setTimeout(() => setStatus(""), 10 * 1000);
    };
    const callBackWithStatus = (msg: string) => {
        showStatus(msg);
        window.refreshBackendData();
    };
    return (
        <div key={id} className="row_container vcentered bottom_spaced">
            {status && <span>{status}</span>}
            {!status && (
                <>
                    <span className="row_container vcentered">
                        <div className="right_half_spaced vcentered">
                            {logo ? (
                                <img
                                    src={`${logo}`}
                                    width="20px"
                                    height="20px"
                                />
                            ) : (
                                <span style={{ width: "20px" }}>💎</span>
                            )}
                        </div>{" "}
                        <a href={`#/${id}`}>{symbol}</a>
                    </span>
                    <div className="max_width_col"></div>
                    <code>{token(balance, decimals)}</code>
                    <Button
                        classNameArg="left_half_spaced"
                        onClick={() =>
                            internal
                                ? withdrawToWallet(
                                      id,
                                      decimals,
                                      callBackWithStatus,
                                  )
                                : withdrawToPrincipal(
                                      id,
                                      fee,
                                      balance,
                                      decimals,
                                      symbol,
                                      callBackWithStatus,
                                  )
                        }
                        label="WITHDRAW"
                    />
                </>
            )}
        </div>
    );
};

const withdrawToWallet = async (
    id: string,
    decimals: number,
    callback: (arg: string) => void,
) => {
    try {
        let result: any = await window.api.withdraw(Principal.fromText(id));
        if ("Err" in result) {
            alert(`Error: ${result.Err}`);
            return;
        }
        if ("Ok" in result)
            callback(`Success! Withdrew ${token(result.Ok, decimals)} tokens.`);
    } catch (e) {
        alert(e);
    }
};
const withdrawToPrincipal = async (
    id: string,
    fee: bigint,
    balance: bigint,
    decimals: number,
    symbol: string,
    callback: (arg: string) => void,
) => {
    const recipient = prompt("Enter the withdrawal principal");
    if (!recipient) return;
    if (
        confirm(
            `Withdrawing ${token(balance, decimals)} ${symbol} (fee: ${token(
                fee,
                decimals,
            )}) to\n\n${recipient}`,
        )
    ) {
        try {
            let result: any = await window.api.transfer(
                Principal.fromText(id),
                Principal.fromText(recipient),
                new Uint8Array(32),
                BigInt(balance) - BigInt(fee),
            );
            if ("Err" in result) {
                alert(`Error: ${result.Err}`);
                return;
            }
            if ("Ok" in result)
                callback(`Success! Transaction ID: ${result.Ok}`);
        } catch (e) {
            alert(e);
        }
    }
};
