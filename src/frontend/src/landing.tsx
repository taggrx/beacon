import { Principal } from "@dfinity/principal";
import { PAYMENT_TOKEN_ID, bigScreen, token } from "./common";
import * as React from "react";

export const Landing = ({}) => {
    const [prices, setPrices] = React.useState<{ [name: string]: bigint }>({});
    const [stats, setStats] = React.useState<{ [name: string]: any }>({});

    const loadData = async () => {
        const [prices, stats] = await Promise.all([
            window.api.query<{ [name: string]: bigint }>("prices"),
            window.api.query<{ [name: string]: any }>("stats"),
        ]);
        if (prices) setPrices(prices);
        if (stats) setStats(stats);
    };

    React.useEffect(() => {
        loadData();
    }, []);
    const paymentToken = window.tokenData[PAYMENT_TOKEN_ID];

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
            <div className={bigScreen() ? "dynamic_table" : "two_columns_grid"}>
                <div className="dbcell">
                    {paymentToken.symbol} LOCKED
                    <code>
                        {token(stats.icp_locked, paymentToken.decimals)}{" "}
                    </code>
                </div>
                <div className="dbcell">
                    24H TRADES
                    <code>{stats.trades_day}</code>
                </div>
                <div className="dbcell">
                    24H VOLUME
                    <code>
                        {token(stats.volume_day, paymentToken.decimals)}{" "}
                        {paymentToken.symbol}
                    </code>
                </div>
                <div className="dbcell">
                    FEES
                    <code>
                        {token(window.data.fee, paymentToken.decimals)}%
                    </code>
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
                            const id = Principal.fromText(
                                prompt("Enter the canister id:") || "",
                            );
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
