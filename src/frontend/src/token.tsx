import * as React from "react";
import { Metadata, Order, OrderType, Result } from "./types";
import { Principal } from "@dfinity/principal";
import {
    Button,
    Error,
    PAYMENT_TOKEN_ID,
    bigScreen,
    token,
    tokenFee,
} from "./common";
import { Listing } from "./listing";

export const Token = ({ id }: { id: string }) => {
    const [metadata, setMetadata] = React.useState<Result<Metadata> | null>();
    const [buyOrders, setBuyOrders] = React.useState<Order[]>([]);
    const [sellOrders, setSellOrders] = React.useState<Order[]>([]);
    const loadData = async (id: string) => {
        const [metadata, buyOrders, sellOrders] = await Promise.all([
            await window.api.query<Result<Metadata>>("token", id),
            await window.api.orders(Principal.fromText(id), OrderType.Buy),
            await window.api.orders(Principal.fromText(id), OrderType.Sell),
        ]);
        setMetadata(metadata);
        setBuyOrders(buyOrders as unknown as any);
        setSellOrders(sellOrders as unknown as any);
    };
    React.useEffect(() => {
        if (id) loadData(id);
    }, []);

    if (!metadata) return <Error text="something went wrong." />;

    if ("Err" in metadata) {
        return <Listing id={id} />;
    }

    const { symbol, logo } = metadata.Ok;
    return (
        <>
            <h1 className="row_container vcentered">
                <img
                    height="50"
                    width="50"
                    src={logo}
                    className="align-middle"
                />
                <code className="max_width_col">{symbol}</code>
            </h1>
            <div
                className={
                    bigScreen() ? "two_columns_grid" : "column_container"
                }
            >
                <OrderMask id={id} symbol={symbol} orderType={OrderType.Buy} />
                <OrderMask id={id} symbol={symbol} orderType={OrderType.Sell} />
            </div>
            <OrderBook sellers={sellOrders} buyers={buyOrders} />
        </>
    );
};

const OrderMask = ({
    id,
    symbol,
    orderType,
}: {
    id: string;
    symbol: string;
    orderType: OrderType;
}) => {
    const [amount, setAmount] = React.useState("0.0");
    const [price, setPrice] = React.useState("");
    const [status, setStatus] = React.useState("");

    const icrcToken = window.tokenData[id];
    const tokenDecimals = icrcToken.decimals;
    const paymentToken = window.tokenData[PAYMENT_TOKEN_ID];

    React.useEffect(() => setStatus(""), [price, amount]);
    const action = orderType.toString().toUpperCase();

    return (
        <div className="column_container bottom_spaced max_width_col">
            <h3>CREATE A {action} ORDER</h3>
            <div className="row_container vcentered bottom_spaced modal">
                TOTAL
                <input
                    type="number"
                    min="0"
                    className="max_width_col"
                    value={amount}
                    onChange={(e) => setAmount(e.target.value)}
                />
                {symbol}
            </div>
            <div className="row_container vcentered bottom_spaced modal">
                LIMIT
                <input
                    type="number"
                    placeholder={
                        orderType == OrderType.Buy
                            ? "MAX PRICE TO PAY"
                            : "MIN PRICE TO ASK"
                    }
                    className="max_width_col"
                    value={price}
                    onChange={(e) => setPrice(e.target.value)}
                />
                {paymentToken.symbol}
            </div>
            {status && <span className="bottom_spaced">{status}</span>}
            <Button
                styleArg={{
                    background: orderType == OrderType.Buy ? "green" : "red",
                }}
                label={`${price ? "LIMIT " : "MARKET "}${action} (FEE ${
                    Number(window.data.fee) / 100
                }%)`}
                onClick={async () => {
                    const parsedAmount = parseNumber(amount, tokenDecimals);
                    if (parsedAmount == null) {
                        setStatus(`🔴 Can't parse the amount "${amount}"`);
                        return;
                    }
                    const parsedPrice = parseNumber(
                        price,
                        paymentToken.decimals,
                    );
                    if (parsedPrice == null) {
                        setStatus(`🔴 Can't parse the price "${price}"`);
                        return;
                    }
                    await executeOrder(
                        id,
                        BigInt(parsedAmount),
                        BigInt(parsedPrice / Math.pow(10, tokenDecimals)),
                        orderType,
                        setStatus,
                    );
                }}
            />
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
        let status = `Order filled for ${filled} tokens. `;
        status += orderCreated
            ? "An order was created."
            : "No order was created.";
        statusCallback(status);
    } catch (error) {
        statusCallback(`🔴 ${error}`);
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

const OrderBook = ({
    sellers,
    buyers,
}: {
    sellers: Order[];
    buyers: Order[];
}) => {
    console.log(sellers, buyers);
    return null;
};
