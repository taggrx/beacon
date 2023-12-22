import * as React from "react";
import { Order, OrderType } from "./types";
import { Principal } from "@dfinity/principal";
import { PAYMENT_TOKEN_ID, humanReadablePrice, orderId, token } from "./common";
import { Listing } from "./listing";
import { OrderMask } from "./order_mask";
import { OrderBook } from "./order_book";

export const Token = ({ tokenId }: { tokenId: string }) => {
    const [executedOrders, setExecutedOrders] = React.useState<Order[]>([]);
    const [heartbeat, setHeartbeat] = React.useState(new Date());
    const [orderCreation, setOrderCreation] = React.useState<OrderType | null>(
        null,
    );
    const loadData = async (tokenId: string) => {
        const executedOrders = await window.api.executed_orders(
            Principal.fromText(tokenId),
        );
        setExecutedOrders(executedOrders as unknown as any);
    };
    React.useEffect(() => {
        if (tokenId) loadData(tokenId);
    }, []);

    const metadata = window.tokenData[tokenId];

    if (!metadata) {
        return <Listing tokenId={tokenId} />;
    }

    const { symbol, logo, realm } = metadata;
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
            <Chart
                tokenId={tokenId}
                prices={executedOrders.map((order) => Number(order.price))}
            />
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
                                    type == OrderType.Buy
                                        ? "#008800"
                                        : "#cc0000",
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
                    <h2>EXECUTED ORDERS</h2>
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
                                    <td style={{ textAlign: "right" }}>
                                        {tokenData.symbol}
                                    </td>
                                    <td style={{ textAlign: "right" }}>
                                        {token(
                                            humanReadablePrice(
                                                order.price,
                                                tokenId,
                                            ),
                                            paymentTokenDataData.decimals,
                                        )}
                                    </td>
                                    <td style={{ textAlign: "right" }}>
                                        {paymentTokenDataData.symbol}
                                    </td>
                                </tr>
                            ))}
                        </tbody>
                    </table>
                </>
            )}
            <br />
            <h2>{realm ? "REALM" : "MENTIONS"} ON TAGGR</h2>
            <iframe
                src={`https://6qfxa-ryaaa-aaaai-qbhsq-cai.ic0.app/#/${
                    realm ? "realm/" + realm : "feed/" + symbol
                }`}
                title={`${symbol} on Taggr`}
            />
        </>
    );
};

const Chart = ({ prices, tokenId }: { prices: number[]; tokenId: string }) => {
    if (prices.length < 5) return null;

    prices.reverse();
    const chartRef = React.useRef(null);

    React.useEffect(() => {
        if (!chartRef.current) return;
        const canvas = chartRef.current as unknown as HTMLCanvasElement;
        const ctx = canvas.getContext("2d");
        if (!ctx) return;
        ctx.clearRect(0, 0, canvas.width, canvas.height);

        let yMax = Math.max(...prices);
        let yMin = Math.min(...prices);
        const scale = Math.max(...prices) - Math.min(...prices);
        if (scale == 0) return;

        const data = prices.map(
            (value: number) => ((value - yMin) / scale) * 100,
        );

        yMax = Math.max(...data);

        const margin = 50;
        const xScale = (canvas.width - 2 * margin) / (data.length - 1);
        const yScale = (canvas.height - 2 * margin) / yMax;

        ctx.lineJoin = "round";
        ctx.lineCap = "round";
        ctx.lineWidth = 2;
        ctx.strokeStyle = "#6ac2c9";
        ctx.font = "18px JetBrains Mono";
        ctx.fillStyle = "white";

        ctx.beginPath();
        ctx.moveTo(margin, canvas.height - margin - data[0] * yScale);
        for (let i = 1; i < data.length; i++) {
            const x = i * xScale + margin;
            const y = canvas.height - margin - data[i] * yScale;
            ctx.lineTo(x, y);
            ctx.fillText(
                token(
                    humanReadablePrice(BigInt(Math.floor(prices[i])), tokenId),
                    window.tokenData[PAYMENT_TOKEN_ID].decimals,
                ).toString(),
                x - 15,
                Math.max(y + 20, 0),
            );
        }
        ctx.stroke();
    }, [prices]);

    return (
        <div
            className="row_container top_spaced bottom_spaced"
            style={{ justifyContent: "center" }}
        >
            <canvas
                width={1024}
                height={400}
                style={{
                    width: "100%",
                    maxWidth: "1000px",
                }}
                ref={chartRef}
            ></canvas>
        </div>
    );
};
