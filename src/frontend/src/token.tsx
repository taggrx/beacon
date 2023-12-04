import * as React from "react";
import { Metadata, Order, OrderType, Result } from "./types";
import { Principal } from "@dfinity/principal";
import {
    Button,
    Error,
    MAINNET_LEDGER_CANISTER_ID,
    token,
    tokenFee,
} from "./common";
import { Listing } from "./listing";

export const Token = ({ id }: { id: string }) => {
    const [metadata, setMetadata] = React.useState<Result<Metadata> | null>();
    const [buyOrders, setBuyOrders] = React.useState<Order[]>([]);
    const [sellOrders, setSellOrders] = React.useState<Order[]>([]);
    const [showOrderMask, toggleOrderMask] = React.useState(false);
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
                <div style={{ opacity: showOrderMask ? "0.5" : undefined }}>
                    <Button
                        onClick={async () => toggleOrderMask(!showOrderMask)}
                        label="NEW ORDER"
                    />
                </div>
            </h1>
            {showOrderMask && <OrderMask id={id} />}
            <OrderBook sellers={sellOrders} buyers={buyOrders} />
        </>
    );
};

const OrderMask = ({ id }: { id: string }) => {
    const [amount, setAmount] = React.useState("0.0");
    const [price, setPrice] = React.useState("");
    const [orderType, setOrderType] = React.useState<OrderType>(OrderType.Buy);
    const [status, setStatus] = React.useState("");

    React.useEffect(() => {
        setAmount("0.0");
        setPrice("0.0");
    }, [orderType]);

    return (
        <div className="column_container modal">
            <div className="row_container vcentered bottom_half_spaced">
                <Button
                    classNameArg="max_width_col right_half_spaced"
                    styleArg={{
                        background: "red",
                        opacity: orderType == OrderType.Buy ? "0.3" : "1",
                    }}
                    label="SELL"
                    onClick={async () => setOrderType(OrderType.Sell)}
                />
                <Button
                    classNameArg="max_width_col left_half_spaced"
                    styleArg={{
                        background: "green",
                        opacity: orderType == OrderType.Sell ? "0.3" : "1",
                    }}
                    label="BUY"
                    onClick={async () => setOrderType(OrderType.Buy)}
                />
            </div>
            <div className="row_container vcentered bottom_half_spaced">
                <span className="max_width_col">
                    AMOUNT (
                    {orderType == OrderType.Buy
                        ? "ICP"
                        : window.tokenData[id].symbol}
                    ):
                </span>
                <div className="max_width_col row_container">
                    <input
                        type="number"
                        min="0"
                        className="max_width_col"
                        value={amount}
                        onChange={(e) => {
                            setAmount(e.target.value);
                        }}
                    />
                    <button
                        className="left_half_spaced"
                        onClick={() => {
                            const tokenID =
                                orderType == OrderType.Buy
                                    ? MAINNET_LEDGER_CANISTER_ID
                                    : id;
                            setAmount(
                                token(
                                    window.balances[tokenID],
                                    window.tokenData[tokenID].decimals,
                                ),
                            );
                        }}
                    >
                        MAX
                    </button>
                </div>
            </div>
            <div className="row_container vcentered bottom_half_spaced">
                <span className="max_width_col">
                    LIMIT PRICE (
                    {orderType == OrderType.Sell
                        ? "ICP"
                        : window.tokenData[id].symbol}
                    ):
                </span>
                <div className="max_width_col row_container">
                    <input
                        type="number"
                        min="0"
                        className="max_width_col"
                        value={price}
                        placeholder={
                            orderType == OrderType.Buy
                                ? "MAX PRICE YOU PAY"
                                : "LOWEST PRICE YOU ACCEPT"
                        }
                        onChange={(e) => setPrice(e.target.value)}
                    />
                </div>
            </div>
            {status && <span className="bottom_half_spaced">{status}</span>}
            <Button
                label={orderType.toString().toUpperCase()}
                onClick={async () => {
                    const parsedAmount = parseAmount(
                        amount,
                        window.tokenData[
                            orderType == OrderType.Buy
                                ? MAINNET_LEDGER_CANISTER_ID
                                : id
                        ].decimals,
                    );
                    if (parsedAmount == null) {
                        setStatus(`🔴 Couldn't parse the amount "${amount}"`);
                        return;
                    }
                    const parsedPrice = parseAmount(
                        price,
                        window.tokenData[
                            orderType == OrderType.Buy
                                ? id
                                : MAINNET_LEDGER_CANISTER_ID
                        ].decimals,
                    );
                    if (parsedPrice == null) {
                        setStatus(`🔴 Couldn't parse the price "${price}"`);
                        return;
                    }
                    await executeOrder(
                        id,
                        BigInt(parsedAmount),
                        BigInt(parsedPrice),
                        orderType,
                        setStatus,
                    );
                }}
            />
        </div>
    );
};

const executeOrder = async (
    token: string,
    amount: bigint,
    price: bigint,
    orderType: OrderType,
    statusCallback: (arg: string) => void,
) => {
    let tokenId = Principal.from(
        orderType == OrderType.Buy ? MAINNET_LEDGER_CANISTER_ID : token,
    );
    // lock funds
    statusCallback("Transferring funds to BEACON...");
    let result: any = await window.api.transfer(
        tokenId,
        Principal.from(process.env.CANISTER_ID),
        window.principalId.toUint8Array(),
        // We need to add fees for a second transfer from the user account into the pool
        (orderType == OrderType.Buy ? amount * price : amount) +
            tokenFee(tokenId.toString()),
    );
    if ("Err" in result) {
        alert(`Error: ${JSON.stringify(result.Err)}`);
        return;
    }
    statusCallback("Executing your trade...");
    result = await window.api.trade(tokenId, amount, price, orderType);
    if ("Err" in result) {
        alert(`Error: ${JSON.stringify(result.Err)}`);
        return;
    }
    let [filled, orderCreated] = result.Ok;
    let status = `Order filled for ${filled} tokens. `;
    status += orderCreated ? "An order was created." : "No order was created.";
    statusCallback(status);
};

function parseAmount(amount: string, tokenDecimals: number): number | null {
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
