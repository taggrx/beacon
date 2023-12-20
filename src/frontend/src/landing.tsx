import { ConnectButton, PAYMENT_TOKEN_ID, token } from "./common";
import * as React from "react";

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

    return (
        <div>
            <div className="text_centered">
                <h1 className="logo">BEACON</h1>
                <h3>
                    <s>Immutable</s> Order-Book Based Exchange
                </h3>
            </div>
            <br />
            {!window.principalId && <ConnectButton />}
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
                        <div className="row_container vcentered bottom_spaced x_large">
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
            </div>
        </div>
    );
};
