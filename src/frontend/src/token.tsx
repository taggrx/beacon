import * as React from "react";
import { Metadata, Order, OrderType, Result } from "./types";
import { Principal } from "@dfinity/principal";
import { Button, Error, PAYMENT_TOKEN_ID, token, tokenFee } from "./common";
import { Listing } from "./listing";

export const Token = ({ tokenId }: { tokenId: string }) => {
    const [metadata, setMetadata] = React.useState<Result<Metadata> | null>();
    const [heartbeat, setHeartbeat] = React.useState(new Date());
    const [orderCreation, setOrderCreation] = React.useState<OrderType | null>(
        null,
    );
    const loadData = async (tokenId: string) => {
        const [metadata] = await Promise.all([
            await window.api.query<Result<Metadata>>("token", tokenId),
        ]);
        setMetadata(metadata);
    };
    React.useEffect(() => {
        if (tokenId) loadData(tokenId);
    }, []);

    if (!metadata) return <Error text="something went wrong." />;

    if ("Err" in metadata) {
        return <Listing tokenId={tokenId} />;
    }

    const { symbol, logo } = metadata.Ok;
    const callback = () => {
        window.refreshBackendData();
        setHeartbeat(new Date());
    };
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
            <OrderBook tokenId={tokenId} heartbeat={heartbeat} />
            {orderCreation && (
                <OrderMask
                    tokenId={tokenId}
                    symbol={symbol}
                    orderType={orderCreation}
                    callback={callback}
                    cancelCallback={() => setOrderCreation(null)}
                />
            )}
            {!orderCreation && (
                <div className="row_container">
                    {[OrderType.Buy, OrderType.Sell].map((type, i) => (
                        <button
                            key={i}
                            style={{
                                color: "white",
                                background:
                                    type == OrderType.Buy ? "green" : "red",
                            }}
                            className={`max_width_col ${
                                type == OrderType.Buy
                                    ? "right_half_spaced"
                                    : "left_half_spaced"
                            }`}
                            onClick={() => setOrderCreation(type)}
                        >
                            {type.toString().toUpperCase()}
                        </button>
                    ))}
                </div>
            )}
        </>
    );
};

const OrderBook = ({
    tokenId,
    heartbeat,
}: {
    tokenId: string;
    heartbeat: any;
}) => {
    const [buyOrders, setBuyOrders] = React.useState<Order[]>([]);
    const [sellOrders, setSellOrders] = React.useState<Order[]>([]);
    const loadData = async () => {
        const [buyOrders, sellOrders] = await Promise.all([
            await window.api.orders(Principal.fromText(tokenId), OrderType.Buy),
            await window.api.orders(
                Principal.fromText(tokenId),
                OrderType.Sell,
            ),
        ]);
        setBuyOrders(buyOrders as unknown as any);
        setSellOrders(sellOrders as unknown as any);
    };

    React.useEffect(() => {
        loadData();
    }, [heartbeat]);

    const maxOrderSize = buyOrders
        .concat(sellOrders)
        .reduce((acc, order) => Math.max(acc, Number(order.amount)), 0);

    const render = (orders: Order[], orderType: OrderType) => {
        return (
            <div
                className="column_container max_width_col bottom_spaced"
                style={{
                    alignItems:
                        orderType == OrderType.Buy ? "flex-end" : "flex-start",
                    paddingLeft: orderType == OrderType.Sell ? "0.4em" : "0",
                    paddingRight: orderType == OrderType.Buy ? "0.4em" : "0",
                }}
            >
                <h3
                    style={{
                        color: orderType == OrderType.Buy ? "green" : "red",
                    }}
                >
                    {orderType == OrderType.Buy ? "BUYERS" : "SELLERS"}
                </h3>
                {orders.map((order, i) => (
                    <div
                        key={i}
                        className="column_container"
                        style={{
                            width: "100%",
                            alignItems:
                                orderType == OrderType.Buy
                                    ? "flex-end"
                                    : "flex-start",
                            fontSize: "xx-small",
                            color: orderType == OrderType.Buy ? "green" : "red",
                            boxSizing: "border-box",
                        }}
                    >
                        <div
                            style={{
                                paddingLeft: "0.5em",
                                paddingRight: "0.5em",
                            }}
                        >
                            {token(
                                BigInt(
                                    Number(order.price) *
                                        Math.pow(
                                            10,
                                            window.tokenData[tokenId].decimals,
                                        ),
                                ),
                                window.tokenData[PAYMENT_TOKEN_ID].decimals,
                            )}{" "}
                            {window.tokenData[PAYMENT_TOKEN_ID].symbol}
                        </div>
                        <div
                            style={{
                                width: `${
                                    (Number(order.amount) / maxOrderSize) * 100
                                }%`,
                                color: "white",
                                fontSize: "xx-small",
                                paddingLeft: "0.5em",
                                paddingRight: "0.5em",
                                boxSizing: "border-box",
                                background:
                                    orderType == OrderType.Buy
                                        ? "green"
                                        : "red",
                            }}
                        >
                            {(Number(order.amount) / maxOrderSize) * 100 >
                            15 ? (
                                `${token(
                                    order.amount,
                                    window.tokenData[tokenId].decimals,
                                )} ${window.tokenData[tokenId].symbol}`
                            ) : (
                                <span>&nbsp;</span>
                            )}
                        </div>
                    </div>
                ))}
            </div>
        );
    };

    return (
        <>
            <div className="row_container">
                {render(buyOrders, OrderType.Buy)}
                {render(sellOrders, OrderType.Sell)}
            </div>
        </>
    );
};

const OrderMask = ({
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
    const [amount, setAmount] = React.useState("0.0");
    const [price, setPrice] = React.useState("");
    const [status, setStatus] = React.useState("");

    const icrcToken = window.tokenData[tokenId];
    const tokenDecimals = icrcToken.decimals;
    const paymentToken = window.tokenData[PAYMENT_TOKEN_ID];

    React.useEffect(() => setStatus(""), [price, amount]);
    const action = orderType.toString().toUpperCase();

    return (
        <div className="column_container bottom_spaced max_width_col">
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
            <div className="row_container">
                <button
                    className="max_width_col right_half_spaced"
                    onClick={cancelCallback}
                >
                    CANCEL
                </button>
                <Button
                    classNameArg="max_width_col left_half_spaced"
                    styleArg={{
                        color: "white",
                        background:
                            orderType == OrderType.Buy ? "green" : "red",
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
                            tokenId,
                            BigInt(parsedAmount),
                            BigInt(parsedPrice / Math.pow(10, tokenDecimals)),
                            orderType,
                            setStatus,
                        );
                        callback();
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
