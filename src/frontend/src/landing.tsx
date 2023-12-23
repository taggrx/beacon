import { Principal } from "@dfinity/principal";
import { ConnectButton, PAYMENT_TOKEN_ID, bigScreen, token } from "./common";
import * as React from "react";
import { Wallet } from "./wallet";

export const Landing = ({}) => {
    const [prices, setPrices] = React.useState<{ [name: string]: bigint }>({});

    const loadData = async () => {
        const prices = await window.api.query<{ [name: string]: bigint }>(
            "prices",
        );
        if (prices) setPrices(prices);
    };

    React.useEffect(() => {
        loadData();
    }, []);
    const paymentToken = window.tokenData[PAYMENT_TOKEN_ID];
    const { icp_locked, trades_day, volume_day, fee } = window.data;

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
            </div>
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
                {Object.entries(window.tokenData)
                    .filter((entry) => entry[0] != PAYMENT_TOKEN_ID)
                    .map(([id, { symbol, logo }]) => (
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
                                    <span style={{ width: "20px" }}>💎</span>
                                )}
                            </div>{" "}
                            <a href={`#/${id}`}>{symbol}</a>
                            <div className="max_width_col"></div>
                            <code>
                                {token(prices[id], paymentToken.decimals)}{" "}
                                {paymentToken.symbol}
                            </code>
                        </div>
                    ))}
                <br />
                <button
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
    );
};
