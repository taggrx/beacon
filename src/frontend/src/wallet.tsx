import * as React from "react";
import { Button, CopyToClipboard, token } from "./common";
import { Principal } from "@dfinity/principal";

export const Wallet = ({}) => {
    const [internalBalances, setInternalBalances] = React.useState<{
        [key: string]: bigint;
    }>({});
    const loadData = async () => {
        let [internalBalances] = await Promise.all([
            window.api.query<{ [key: string]: bigint }>("token_balances"),
        ]);
        setInternalBalances(internalBalances || {});
    };

    React.useEffect(() => {
        loadData();
    }, []);

    const internalRenderedBalances = renderBalances(
        internalBalances,
        loadData,
        "internal",
    );

    return (
        <div id="wallet" className="modal column_container">
            <h3>FUNDS IN WALLET</h3>
            {renderBalances(window.balances, loadData)}
            {internalRenderedBalances.length > 0 && (
                <>
                    <h3>FUNDS ON BEACON</h3>
                    {internalRenderedBalances}
                </>
            )}
            <h3>PRINCIPAL</h3>
            <span style={{ fontSize: "small" }}>
                <CopyToClipboard value={window.principalId.toString()} />
            </span>
            <Button
                classNameArg="top_spaced"
                label="REFRESH"
                onClick={loadData}
            />
        </div>
    );
};

const renderBalances = (
    balances: { [key: string]: bigint },
    callback: () => Promise<any>,
    internal?: string,
) =>
    Object.entries(window.tokenData)
        .filter(([id]) => id in balances)
        .map(([id, data]) => (
            <BalanceLine
                key={id}
                id={id}
                logo={data.logo}
                symbol={data.symbol}
                balance={balances[id]}
                decimals={data.decimals}
                fee={data.fee}
                callback={callback}
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
    callback,
    internal,
}: {
    id: string;
    logo: string;
    symbol: string;
    balance: bigint;
    decimals: number;
    fee: bigint;
    internal: boolean;
    callback: () => Promise<any>;
}) => {
    const [status, setStatus] = React.useState("");
    const showStatus = (msg: string) => {
        setStatus(msg);
        setTimeout(() => setStatus(""), 10 * 1000);
    };
    const callBackWithStatus = (msg: string) => {
        showStatus(msg);
        callback();
    };
    return (
        <div key={id} className="row_container vcentered bottom_half_spaced">
            {status && <span>{status}</span>}
            {!status && (
                <>
                    <span className="row_container vcentered">
                        {logo ? (
                            <img src={`${logo}`} width="20px" height="20px" />
                        ) : (
                            <span style={{ width: "20px" }}>💎</span>
                        )}{" "}
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
