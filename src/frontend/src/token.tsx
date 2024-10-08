import * as React from "react";
import { Order, OrderType } from "./types";
import { orderId, token, timeAgo, TokenLogo, paymentTokenData } from "./common";
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
        const executedOrders = await window.api.query(
            "executed_orders",
            tokenId,
        );
        setExecutedOrders(executedOrders as unknown as any);
    };
    React.useEffect(() => {
        if (tokenId) loadData(tokenId);
    }, [tokenId]);

    const tokenData = window.tokenData[tokenId];

    if (!tokenData) {
        return <Listing tokenId={tokenId} />;
    }

    const { symbol, decimals, logo } = tokenData;
    const callback = () => {
        window.refreshBackendData();
        setHeartbeat(new Date());
        loadData(tokenId);
    };
    return (
        <>
            <div className="row_container vcentered x_large_text bottom_spaced">
                <div className="max_width_col vcentered">
                    {logo ? (
                        <img
                            height="50"
                            width="50"
                            src={logo}
                            className="align-middle right_spaced"
                        />
                    ) : (
                        <TokenLogo />
                    )}
                    <code className="left_half_spaced max_width_col">
                        {symbol}
                    </code>
                </div>
                {executedOrders.length > 0 && (
                    <code>
                        {token(
                            executedOrders[0].price,
                            paymentTokenData().decimals,
                        )}{" "}
                        {paymentTokenData().symbol}
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
                    closeCallback={() => setOrderCreation(null)}
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
                            {executedOrders.slice(0, 30).map((order) => (
                                <tr key={orderId(order)}>
                                    <td>
                                        <Timestamp value={order.executed} />
                                    </td>
                                    <td>
                                        <code>
                                            {token(order.amount, decimals)}
                                        </code>{" "}
                                        {symbol}
                                    </td>
                                    <td
                                        style={{
                                            opacity: "0.3",
                                            textAlign: "center",
                                        }}
                                    >
                                        @
                                    </td>
                                    <td style={{ textAlign: "right" }}>
                                        <code>
                                            {token(
                                                order.price,
                                                paymentTokenData().decimals,
                                            )}
                                        </code>{" "}
                                        {paymentTokenData().symbol}
                                    </td>
                                </tr>
                            ))}
                        </tbody>
                    </table>
                </>
            )}
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
        const volumes = orders.map((order) => Number(order.amount));
        let yMax = Math.max(...prices);
        let yMin = Math.min(...prices);
        const scale = Math.max(...prices) - Math.min(...prices);
        if (scale == 0) return;

        const data = prices.map(
            (value: number) => ((value - yMin) / scale) * 100,
        );

        yMax = Math.max(...data);

        const margin = 50;
        const marginY = 70;
        const xScale = (canvas.width - 2 * margin) / (data.length - 1);
        const yScale = (canvas.height - 2 * margin) / yMax;

        ctx.lineJoin = "round";
        ctx.lineCap = "round";
        ctx.lineWidth = 2;
        ctx.strokeStyle = "#6ac2c9";
        ctx.font = "12px JetBrains Mono";
        ctx.fillStyle = "white";

        ctx.beginPath();
        ctx.moveTo(margin, canvas.height - margin - data[0] * yScale);
        for (let i = 1; i < data.length; i++) {
            const x = i * xScale + margin;
            const y = canvas.height - marginY - data[i] * yScale;
            ctx.lineTo(x, y);
            if (i == 1 || i == data.length - 1)
                ctx.fillText(
                    token(
                        orders[i].price,
                        paymentTokenData().decimals,
                    ).toString(),
                    x - 15,
                    Math.max(y + 20, 0),
                );
        }
        ctx.stroke();

        // Draw volumes
        const maxVolume = Math.max(...volumes);
        const maxBarHeight = 40;
        for (let i = 1; i < volumes.length; i++) {
            const x = i * xScale + margin;
            const y = (volumes[i] / maxVolume) * maxBarHeight;
            ctx.beginPath();
            ctx.moveTo(x, canvas.height);
            ctx.lineTo(x, canvas.height - y);
            ctx.stroke();
        }
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
