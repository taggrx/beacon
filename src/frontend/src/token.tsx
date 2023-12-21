import * as React from "react";
import { Metadata, Order, OrderType, Result } from "./types";
import { Principal } from "@dfinity/principal";
import { Button, Error, PAYMENT_TOKEN_ID, token, tokenFee } from "./common";
import { Listing } from "./listing";

const orderId = (order: Order) =>
    order.owner.toString() +
    order.price.toString() +
    order.timestamp +
    order.amount +
    order.executed;

export const Token = ({ tokenId }: { tokenId: string }) => {
    const [metadata, setMetadata] = React.useState<Result<Metadata> | null>();
    const [executedOrders, setExecutedOrders] = React.useState<Order[]>([]);
    const [heartbeat, setHeartbeat] = React.useState(new Date());
    const [orderCreation, setOrderCreation] = React.useState<OrderType | null>(
        null,
    );
    const loadData = async (tokenId: string) => {
        const [metadata, executedOrders] = await Promise.all([
            await window.api.query<Result<Metadata>>("token", tokenId),
            await window.api.executed_orders(Principal.fromText(tokenId)),
        ]);
        setMetadata(metadata);
        setExecutedOrders(executedOrders as unknown as any);
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
        loadData(tokenId);
    };
    const tokenData = window.tokenData[tokenId];
    const paymentTokenDataData = window.tokenData[PAYMENT_TOKEN_ID];
    return (
        <>
            <h1 className="row_container vcentered">
                <div className="max_width_col">
                    <img
                        height="50"
                        width="50"
                        src={logo}
                        className="align-middle"
                    />
                    <code className="max_width_col">{symbol}</code>
                </div>
                {executedOrders.length > 0 && (
                    <code>
                        {token(
                            humanReadablePrice(
                                executedOrders[0].price,
                                tokenId,
                            ),
                            paymentTokenDataData.decimals,
                        )}{" "}
                        {paymentTokenDataData.symbol}
                    </code>
                )}
            </h1>
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
            <OrderBook
                tokenId={tokenId}
                heartbeat={heartbeat}
                callback={callback}
            />
            {executedOrders.length > 0 && (
                <>
                    <h2>Executed Orders</h2>
                    <table className="small_text" style={{ width: "100%" }}>
                        <tbody>
                            {executedOrders.map((order) => (
                                <tr key={orderId(order)}>
                                    <td>
                                        {new Date(
                                            Number(order.executed) / 1000000,
                                        ).toLocaleString()}
                                    </td>
                                    <td>
                                        {token(
                                            order.amount,
                                            tokenData.decimals,
                                        )}
                                    </td>
                                    <td>{tokenData.symbol}</td>
                                    <td>
                                        {token(
                                            humanReadablePrice(
                                                order.price,
                                                tokenId,
                                            ),
                                            paymentTokenDataData.decimals,
                                        )}
                                    </td>
                                    <td>{paymentTokenDataData.symbol}</td>
                                </tr>
                            ))}
                        </tbody>
                    </table>
                </>
            )}
        </>
    );
};

const humanReadablePrice = (price: bigint, tokenId: string) =>
    BigInt(Number(price) * Math.pow(10, window.tokenData[tokenId].decimals));

const OrderBook = ({
    tokenId,
    heartbeat,
    callback,
}: {
    tokenId: string;
    heartbeat: any;
    callback: () => void;
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

    const filter = (order: Order) =>
        order.owner.toString() == window.principalId?.toString();
    const userOrders = {
        buy: buyOrders.filter(filter),
        sell: sellOrders.filter(filter),
    };

    const tokenData = window.tokenData[tokenId];
    const paymentTokenDataData = window.tokenData[PAYMENT_TOKEN_ID];

    const userOrdersList = (orders: Order[], type: OrderType) =>
        orders.map((order) => (
            <tr
                key={orderId(order)}
                className="row_container small_text vcentered"
            >
                <td
                    style={{
                        color: type == OrderType.Buy ? "green" : "red",
                    }}
                >
                    {type.toString().toUpperCase()}
                </td>
                <td>
                    {token(order.amount, tokenData.decimals)} {tokenData.symbol}
                </td>
                <td>
                    {token(order.price, paymentTokenDataData.decimals)}{" "}
                    {paymentTokenDataData.symbol}
                </td>
                <td style={{ textAlign: "right" }}>
                    <Button
                        onClick={async () => {
                            await window.api.close_order(
                                Principal.fromText(tokenId),
                                type,
                                order.amount,
                                order.price,
                                order.timestamp,
                            );
                            await loadData();
                            callback();
                        }}
                        label="CLOSE"
                    />
                </td>
            </tr>
        ));

    const render = (orders: Order[], orderType: OrderType) =>
        orders.length == 0 ? null : (
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
                                humanReadablePrice(order.price, tokenId),
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
                            20 ? (
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

    return (
        <>
            <div className="row_container">
                {render(buyOrders, OrderType.Buy)}
                {render(sellOrders, OrderType.Sell)}
            </div>
            {(userOrders.buy.length > 0 || userOrders.sell.length > 0) && (
                <>
                    <h3>YOUR PENDING ORDERS</h3>
                    <table>
                        <tbody>
                            {userOrdersList(userOrders.buy, OrderType.Buy)}
                            {userOrdersList(userOrders.sell, OrderType.Sell)}
                        </tbody>
                    </table>
                    <br />
                </>
            )}
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
    const [blocked, setBlocked] = React.useState(false);
    const [price, setPrice] = React.useState("");
    const [status, setStatus] = React.useState("");

    const icrcToken = window.tokenData[tokenId];
    const tokenDecimals = icrcToken.decimals;
    const paymentToken = window.tokenData[PAYMENT_TOKEN_ID];

    React.useEffect(() => setStatus(""), [price, amount]);
    const action = orderType.toString().toUpperCase();

    return (
        <div className="column_container bottom_spaced max_width_col">
            <div style={{ opacity: blocked ? "0.5" : undefined }}>
                <div className="row_container vcentered bottom_spaced modal">
                    TOTAL
                    <input
                        type="number"
                        disabled={blocked}
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
                        disabled={blocked}
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
                    label={`${price ? "LIMIT " : "MARKET "}${action}`}
                    onClick={async () => {
                        setBlocked(true);
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
