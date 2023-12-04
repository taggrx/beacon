import * as React from "react";
import { Button, CopyToClipboard, token } from "./common";
import { Principal } from "@dfinity/principal";
import { Metadata } from "./types";

export const Wallet = ({}) => {
    const [metadata, setMetadata] = React.useState<{ [key: string]: Metadata }>(
        {},
    );
    const [balances, setBalances] = React.useState<{ [key: string]: bigint }>(
        {},
    );
    const [internalBalances, setInternalBalances] = React.useState<{
        [key: string]: bigint;
    }>({});
    const loadData = async () => {
        let [metadata, internalBalances] = await Promise.all([
            window.api.query<{ [key: string]: Metadata }>("tokens"),
            window.api.query<{ [key: string]: bigint }>("token_balances"),
        ]);
        setMetadata(metadata || {});
        setInternalBalances(internalBalances || {});
        Object.keys(metadata || {}).forEach(async (tokenId) => {
            (balances || {})[tokenId] = await window.api.account_balance(
                Principal.fromText(tokenId),
                window.principalId,
            );
            setBalances({ ...balances });
        });
    };

    React.useEffect(() => {
        loadData();
    }, []);

    return (
        <div id="wallet" className="column_container">
            <h3>FUNDS IN WALLET</h3>
            {renderBalances(metadata, balances, loadData)}
            <h3>FUNDS ON BEACON</h3>
            {renderBalances(metadata, internalBalances, loadData)}
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
    metadata: { [key: string]: Metadata },
    balances: { [key: string]: bigint },
    refreshCallback: () => Promise<any>,
) =>
    Object.entries(metadata)
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
                refreshCallback={refreshCallback}
            />
        ));

const BalanceLine = ({
    id,
    logo,
    symbol,
    balance,
    decimals,
    fee,
    refreshCallback,
}: {
    id: string;
    logo: string;
    symbol: string;
    balance: bigint;
    decimals: number;
    fee: bigint;
    refreshCallback: () => Promise<any>;
}) => {
    return (
        <div key={id} className="row_container vcentered bottom_half_spaced">
            <span className="row_container vcentered">
                {logo ? (
                    <img src={`${logo}`} width="20px" height="20px" />
                ) : (
                    "💎"
                )}{" "}
                {symbol}
            </span>
            <div className="max_width_col"></div>
            <code>{token(balance, decimals)}</code>
            <Button
                classNameArg="left_half_spaced"
                onClick={async () => {
                    const recipient = prompt("Enter the withdrawal principal");
                    if (!recipient) return;
                    if (
                        confirm(
                            `Withdrawing ${token(
                                balance,
                                decimals,
                            )} ${symbol} (fee: ${token(
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
                                balance - BigInt(fee),
                            );
                            await refreshCallback();
                            if ("Err" in result) {
                                alert(`Error: ${result.Err}`);
                                return;
                            }
                            if ("Ok" in result)
                                alert(`Success! Transaction ID: ${result.Ok}`);
                        } catch (e) {
                            alert(e);
                        }
                    }
                }}
                label="WITHDRAW"
            />
        </div>
    );
};
