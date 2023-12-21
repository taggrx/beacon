import * as React from "react";
import { Metadata, Order, OrderType, Result } from "./types";
import { Principal } from "@dfinity/principal";
import {
    Error,
    PAYMENT_TOKEN_ID,
    humanReadablePrice,
    orderId,
    token,
} from "./common";
import { Listing } from "./listing";
import { OrderMask } from "./order_mask";
import { OrderBook } from "./order_book";

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
                    <table
                        className="small_text bottom_spaced"
                        style={{ width: "100%" }}
                    >
                        <tbody>
                            {executedOrders.map((order) => (
                                <tr key={orderId(order)}>
                                    <td>
                                        {new Date(
                                            Number(order.executed) / 1000000,
                                        ).toLocaleString()}
                                    </td>
                                    <td style={{ textAlign: "right" }}>
                                        {token(
                                            order.amount,
                                            tokenData.decimals,
                                        )}
                                    </td>
                                    <td>{tokenData.symbol}</td>
                                    <td style={{ textAlign: "right" }}>
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
