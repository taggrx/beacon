import { Principal } from "@dfinity/principal";
import {
    ConnectButton,
    PAYMENT_TOKEN_ID,
    TokenLogo,
    bigScreen,
    token,
} from "./common";
import * as React from "react";
import { Wallet } from "./wallet";
import { Metadata, Order } from "./types";

export const Landing = ({}) => {
    const [orders, setOrders] = React.useState<{ [name: string]: Order }>({});
    const [shortenList, setShortenList] = React.useState(false);

    const loadData = async () => {
        const orders = await window.api.query<{ [name: string]: Order }>(
            "prices",
        );
        if (orders) setOrders(orders);
        setShortenList(Object.keys(window.tokenData).length > 5);
    };

    React.useEffect(() => {
        loadData();
    }, []);
    const paymentToken = window.tokenData[PAYMENT_TOKEN_ID];
    const {
        icp_locked,
        trades_day,
        volume_day,
        fee,
        cycle_balance,
        heap_size,
        tokens_listed,
        active_traders,
    } = window.data;

    const timestamp = (id: string) => (id in orders ? orders[id].timestamp : 0);
    const tokenList = (inputs: [string, Metadata][]) => {
        inputs.sort(([id1], [id2]) => timestamp(id2) - timestamp(id1));
        return shortenList ? inputs.slice(0, 5) : inputs;
    };

    return (
        <div>
            <div className="text_centered">
                <h1 className="logo">BEACON</h1>
                <h3>
                    <s>IMMUTABLE</s> ORDER-BOOK BASED EXCHANGE
                </h3>
                <h4 className="alert">
                    ALPHA VERSION: DON'T USE WITH LARGE AMOUNTS!
                </h4>
            </div>
            <br />
            <div className="row_container" style={{ justifyContent: "center" }}>
                {window.principalId ? (
                    <Wallet />
                ) : (
                    <ConnectButton large={true} />
                )}
            </div>
            <br />
            <br />
            <div className={bigScreen() ? "dynamic_table" : "two_columns_grid"}>
                <div className="dbcell">
                    <span>{paymentToken.symbol} LOCKED</span>
                    <code>
                        {token(icp_locked, paymentToken.decimals, false)}{" "}
                    </code>
                </div>
                <div className="dbcell">
                    <span>24H TRADES</span>
                    <code>{trades_day}</code>
                </div>
                <div className="dbcell">
                    <span>24H VOLUME</span>
                    <code>
                        {token(volume_day, paymentToken.decimals, false)}{" "}
                        {paymentToken.symbol}
                    </code>
                </div>
                <div className="dbcell">
                    <span>FEES</span>
                    <code>{Number(fee) / 100}%</code>
                </div>
                <div className="dbcell">
                    <span>TOKENS LISTED</span>
                    <code>{tokens_listed}</code>
                </div>
                <div className="dbcell">
                    <span>ACTIVE TRADERS</span>
                    <code>{active_traders}</code>
                </div>
                <div className="dbcell">
                    <span>CYCLE BALANCE</span>
                    <code>
                        {(Number(cycle_balance) / 10 ** 12).toLocaleString()} T
                    </code>
                </div>
                <div className="dbcell">
                    <span>HEAP SIZE</span>
                    <code>
                        {(heap_size / 1024 / 1024).toLocaleString(undefined, {
                            minimumFractionDigits: 2,
                        })}{" "}
                        MB
                    </code>
                </div>
            </div>
            <br />
            <br />
            <br />
            <div
                className="column_container"
                style={{
                    width: "80%",
                    marginLeft: "auto",
                    marginRight: "auto",
                }}
            >
                {tokenList(
                    Object.entries(window.tokenData).filter(
                        (entry) => entry[0] != PAYMENT_TOKEN_ID,
                    ),
                ).map(([id, { symbol, logo }]) => (
                    <div
                        key={id}
                        className="row_container vcentered bottom_spaced x_large"
                    >
                        <div className="right_half_spaced vcentered">
                            {logo ? (
                                <img
                                    src={`${logo}`}
                                    width="20px"
                                    height="20px"
                                />
                            ) : (
                                <TokenLogo />
                            )}
                        </div>{" "}
                        <a href={`#/${id}`}>{symbol}</a>
                        <div className="max_width_col"></div>
                        <code>
                            {orders[id]
                                ? token(orders[id].price, paymentToken.decimals)
                                : 0}{" "}
                            {paymentToken.symbol}
                        </code>
                    </div>
                ))}
                <br />
                <div className="row_container">
                    {shortenList && (
                        <div className="text_centered max_width_col">
                            <button onClick={() => setShortenList(false)}>
                                SHOW ALL
                            </button>
                        </div>
                    )}
                    <button
                        className="max_width_col"
                        onClick={() => {
                            try {
                                const input =
                                    prompt("Enter the canister id:") || "";
                                if (!input) return;
                                const id = Principal.fromText(input);
                                if (!id) return;
                                location.href = `#/${id.toString()}`;
                            } catch (e) {
                                alert(e);
                            }
                        }}
                    >
                        LIST YOUR TOKEN NOW!
                    </button>
                </div>
            </div>
        </div>
    );
};
