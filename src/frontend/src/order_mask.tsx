import * as React from "react";
import { OrderType } from "./types";
import { Principal } from "@dfinity/principal";
import { Button, PAYMENT_TOKEN_ID, token, tokenFee } from "./common";

export const OrderMask = ({
    tokenId,
    symbol,
    orderType,
    callback,
    cancelCallback,
}: {
    tokenId: string;
    symbol: string;
    orderType: OrderType;
    callback: () => void;
    cancelCallback: () => void;
}) => {
    const [status, setStatus] = React.useState<string | JSX.Element>("");
    const [blocked, setBlocked] = React.useState(false);
    const [price, setPrice] = React.useState("");
    const [amount, setAmount] = React.useState("0.0");
    const [parsedPrice, setParsedPrice] = React.useState(0);
    const [parsedAmount, setParsedAmount] = React.useState(0);

    const icrcToken = window.tokenData[tokenId];
    const tokenDecimals = icrcToken.decimals;
    const paymentToken = window.tokenData[PAYMENT_TOKEN_ID];

    React.useEffect(() => {
        const parsedAmount = parseNumber(amount, tokenDecimals);
        if (parsedAmount == null) {
            setStatus(`🔴 Can't parse the amount "${amount}"`);
            return;
        }
        setParsedAmount(parsedAmount);
        const parsedPrice = parseNumber(price, paymentToken.decimals);
        if (parsedPrice == null) {
            setStatus(`🔴 Can't parse the price "${price}"`);
            return;
        }
        setParsedPrice(parsedPrice);
        setStatus(
            <span>
                {orderType.toString().toUpperCase()}{" "}
                <code>{token(BigInt(parsedAmount), tokenDecimals)}</code>{" "}
                <u>{icrcToken.symbol}</u>{" "}
                {parsedPrice == 0 ? (
                    "AT MARKET PRICE"
                ) : (
                    <span>
                        FOR{" "}
                        <code>
                            {token(BigInt(parsedPrice), paymentToken.decimals)}
                        </code>{" "}
                        <u>{paymentToken.symbol}</u>
                    </span>
                )}
                {` (FEE ${Number(window.data.fee) / 100}%)`}
            </span>,
        );
    }, [price, amount]);
    const action = orderType.toString().toUpperCase();

    return (
        <div className="column_container bottom_spaced max_width_col">
            <div style={{ opacity: blocked ? "0.5" : undefined }}>
                <div className="row_container vcentered bottom_spaced modal">
                    TOTAL
                    <input
                        disabled={blocked}
                        min="0"
                        className="max_width_col"
                        value={amount}
                        onChange={(e) => setAmount(e.target.value)}
                    />
                    <span style={{ width: "5em", textAlign: "left" }}>
                        {symbol}
                    </span>
                </div>
                <div className="row_container vcentered bottom_spaced modal">
                    LIMIT
                    <input
                        disabled={blocked}
                        placeholder={
                            orderType == OrderType.Buy
                                ? "BID PRICE"
                                : "ASK PRICE"
                        }
                        className="max_width_col"
                        value={price}
                        onChange={(e) => setPrice(e.target.value)}
                    />
                    <span style={{ width: "5em", textAlign: "left" }}>
                        {paymentToken.symbol}
                    </span>
                </div>
            </div>
            {status && parsedAmount > 0 && (
                <span
                    className="small_text bottom_spaced"
                    style={{ textAlign: "right" }}
                >
                    {status}
                </span>
            )}
            <div className="row_container">
                <button
                    className="max_width_col right_half_spaced"
                    onClick={cancelCallback}
                >
                    CANCEL
                </button>
                <Button
                    classNameArg="max_width_col left_half_spaced"
                    disabled={parsedAmount == 0}
                    styleArg={{
                        color: "white",
                        background:
                            orderType == OrderType.Buy ? "green" : "red",
                    }}
                    label={`${price ? "LIMIT " : "MARKET "}${action}`}
                    onClick={async () => {
                        setBlocked(true);
                        await executeOrder(
                            tokenId,
                            BigInt(parsedAmount),
                            BigInt(parsedPrice / Math.pow(10, tokenDecimals)),
                            orderType,
                            setStatus,
                        );
                        callback();
                        setBlocked(false);
                    }}
                />
            </div>
        </div>
    );
};

const executeOrder = async (
    tradedTokenId: string,
    amount: bigint,
    price: bigint,
    orderType: OrderType,
    statusCallback: (arg: string) => void,
) => {
    const paymentTokenId =
        orderType == OrderType.Buy ? PAYMENT_TOKEN_ID : tradedTokenId;
    const paymentToken = window.tokenData[paymentTokenId];
    const balance =
        BigInt(window.balances[paymentTokenId]) - tokenFee(paymentTokenId);
    if (balance > 0) {
        statusCallback(
            `Transferring ${token(balance, paymentToken.decimals)} ${
                paymentToken.symbol
            } to BEACON...`,
        );
        let result: any = await window.api.transfer(
            Principal.fromText(paymentTokenId),
            Principal.from(process.env.CANISTER_ID),
            window.principalId.toUint8Array(),
            balance,
        );
        if ("Err" in result) {
            console.error(result.Err);
            statusCallback("🔴 Transfer to BECAON failed.");
            return;
        }
    }
    statusCallback("Executing your trade...");
    try {
        let result: any = await window.api.trade(
            Principal.from(tradedTokenId),
            amount,
            price,
            orderType,
        );
        if ("Err" in result) {
            console.error(result.Err);
            statusCallback(`Error: ${JSON.stringify(result.Err)}`);
            return;
        }
        let [filled, orderCreated] = result.Ok;
        const { decimals, symbol } = window.tokenData[tradedTokenId];
        let status =
            filled > 0
                ? `Order filled for ${token(filled, decimals)} ${symbol}. `
                : "";
        status += orderCreated
            ? "An order was created."
            : "No order was created.";
        statusCallback(status);
    } catch (error) {
        console.debug(error);
        const regex = /'(.*?)'/g;
        let errorMessage = regex.exec(`${error}`);
        statusCallback(
            `🔴 Error: ${errorMessage?.length ? errorMessage[1] : error}`,
        );
    }
};

function parseNumber(amount: string, tokenDecimals: number): number | null {
    const parse = (s: string): number | null => {
        let num = Number(s);
        if (isNaN(num)) {
            return null;
        }
        return num;
    };

    const tokens = amount.split(".");
    switch (tokens.length) {
        case 1:
            const parsedToken = parse(tokens[0]);
            return parsedToken !== null
                ? parsedToken * Math.pow(10, tokenDecimals)
                : null;
        case 2:
            let afterComma = tokens[1];
            while (afterComma.length < tokenDecimals) {
                afterComma = afterComma + "0";
            }
            afterComma = afterComma.substring(0, tokenDecimals);
            const parsedTokens = parse(tokens[0]);
            const parsedAfterComma = parse(afterComma);
            return parsedTokens !== null && parsedAfterComma !== null
                ? parsedTokens * Math.pow(10, tokenDecimals) + parsedAfterComma
                : null;
        default:
            return null;
    }
}
