import * as React from "react";
import { Button, CopyToClipboard, bigScreen, token } from "./common";
import { Principal } from "@dfinity/principal";

export const Wallet = ({}) => {
    const internalRenderedBalances = renderBalances(
        Object.entries(window.internalBalances).reduce(
            (acc, [id, [balance]]) => {
                acc[id] = balance;
                return acc;
            },
            {} as { [id: string]: bigint },
        ),
        "internal",
    );
    const lockedRenderedBalances = renderBalances(
        Object.entries(window.internalBalances).reduce(
            (acc, [id, [_, locked]]) => {
                acc[id] = locked;
                return acc;
            },
            {} as { [id: string]: bigint },
        ),
        "locked",
    );
    const renderedBalances = renderBalances(window.balances);
    return (
        <div id="wallet" className="modal column_container small_text">
            <h2 className="row_container vcentered">
                <span className="max_width_col">WALLET</span>
                <Button label="REFRESH" onClick={window.refreshBackendData} />
                <Button
                    label="LOGOUT"
                    onClick={async () => {
                        await window.authClient.logout();
                        location.reload();
                    }}
                />
            </h2>
            <div className="row_container vcentered bottom_spaced">
                <span className="max_width_col">PRINCIPAL:</span>
                <CopyToClipboard
                    value={window.principalId.toString()}
                    displayMap={
                        bigScreen()
                            ? undefined
                            : (value: string) => {
                                  const parts = value.split("-");
                                  return `${parts[0]}...${
                                      parts[parts.length - 1]
                                  }`;
                              }
                    }
                />
            </div>
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
            {lockedRenderedBalances.length > 0 && (
                <>
                    <h3>LOCKED IN ORDERS</h3>
                    {lockedRenderedBalances}
                </>
            )}
        </div>
    );
};

const renderBalances = (balances: { [key: string]: bigint }, mark?: string) =>
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
                mark={mark}
            />
        ));

const BalanceLine = ({
    id,
    logo,
    symbol,
    balance,
    decimals,
    fee,
    mark,
}: {
    id: string;
    logo: string;
    symbol: string;
    balance: bigint;
    decimals: number;
    fee: bigint;
    mark?: string;
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
                    {mark != "locked" && (
                        <Button
                            classNameArg="left_half_spaced"
                            onClick={() =>
                                mark == "internal"
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
                    )}
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
            callback(`SUCCESS! WITHDREW ${token(result.Ok, decimals)} TOKENS.`);
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
            `WITHDRAWING ${token(balance, decimals)} ${symbol} (FEE: ${token(
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
                callback(`SUCCESS! TRANSACTION ID: ${result.Ok}`);
        } catch (e) {
            alert(e);
        }
    }
};
