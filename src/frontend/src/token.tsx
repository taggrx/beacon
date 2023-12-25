import * as React from "react";
import { Order, OrderType } from "./types";
import { Principal } from "@dfinity/principal";
import { PAYMENT_TOKEN_ID, orderId, token, timeAgo } from "./common";
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

    const { symbol, logo } = metadata;
    const callback = () => {
        window.refreshBackendData();
        setHeartbeat(new Date());
        loadData(tokenId);
    };
    const tokenData = window.tokenData[tokenId];
    const paymentTokenDataData = window.tokenData[PAYMENT_TOKEN_ID];
    return (
        <>
            <div className="row_container vcentered x_large_text bottom_spaced">
                <div className="max_width_col">
                    {logo ? (
                        <img
                            height="50"
                            width="50"
                            src={logo}
                            className="align-middle right_spaced"
                        />
                    ) : (
                        <span>💎 </span>
                    )}
                    <code className="max_width_col">{symbol}</code>
                </div>
                {executedOrders.length > 0 && (
                    <code>
                        {token(
                            executedOrders[0].price,
                            paymentTokenDataData.decimals,
                        )}{" "}
                        {paymentTokenDataData.symbol}
                    </code>
                )}
            </div>
            <Chart originalOrders={executedOrders} />
            {window.principalId && orderCreation && (
                <OrderMask
                    tokenId={tokenId}
                    symbol={symbol}
                    orderType={orderCreation}
                    callback={callback}
                    cancelCallback={() => setOrderCreation(null)}
                />
            )}
            {window.principalId && !orderCreation && (
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
                        className={"small_text bottom_spaced"}
                        style={{ width: "100%" }}
                    >
                        <tbody>
                            {executedOrders.map((order) => (
                                <tr key={orderId(order)}>
                                    <td>
                                        <Timestamp value={order.executed} />
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
                                            order.price,
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
            <h2>MENTIONS ON TAGGR</h2>
            <iframe
                src={`https://6qfxa-ryaaa-aaaai-qbhsq-cai.ic0.app/#/feed/${symbol}`}
                title={`${symbol} on Taggr`}
            />
        </>
    );
};

const Chart = ({ originalOrders }: { originalOrders: Order[] }) => {
    if (originalOrders.length < 5) return null;
    const orders = [...originalOrders];
    orders.reverse();

    const chartRef = React.useRef(null);

    React.useEffect(() => {
        if (!chartRef.current) return;
        const canvas = chartRef.current as unknown as HTMLCanvasElement;
        const ctx = canvas.getContext("2d");
        if (!ctx) return;
        ctx.clearRect(0, 0, canvas.width, canvas.height);

        const prices = orders.map((order) => Number(order.price));
        let yMax = Math.max(...prices);
        let yMin = Math.min(...prices);
        const scale = Math.max(...prices) - Math.min(...prices);
        if (scale == 0) return;

        const data = prices.map(
            (value: number) => ((value - yMin) / scale) * 100,
        );

        const skipLablesNum = Math.floor(data.length / 6);

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
            if (i == 1 || i == data.length - 1 || i % skipLablesNum == 0)
                ctx.fillText(
                    token(
                        orders[i].price,
                        window.tokenData[PAYMENT_TOKEN_ID].decimals,
                    ).toString(),
                    x - 15,
                    Math.max(y + 20, 0),
                );
        }
        ctx.stroke();
    }, [orders]);

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

const Timestamp = ({ value }: { value: number }) => {
    const [collapsed, setCollapsed] = React.useState(true);
    if (!collapsed) return new Date(Number(value) / 1000000).toLocaleString();
    return (
        <span className="clickable" onClick={() => setCollapsed(false)}>
            {timeAgo(value)}
        </span>
    );
};
