import { Principal } from "@dfinity/principal";
import {
    ConnectButton,
    TokenLogo,
    bigScreen,
    paymentTokenData,
    paymentTokenId,
    token,
} from "./common";
import * as React from "react";
import { Wallet } from "./wallet";
import { Metadata, Order } from "./types";
// @ts-ignore
import readme from "../../../README.md";

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
    const paymentToken = paymentTokenData();
    const {
        payment_token_locked,
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
        <>
            <div className="text_centered">
                <h1 className="logo">BEACON</h1>
                <h3>
                    <s>IMMUTABLE</s> ORDER-BOOK BASED EXCHANGE
                </h3>
                <a href="https://cetrr-jaaaa-aaaak-afgxq-cai.icp0.io">ALPHA</a>{" "}
                &middot;{" "}
                <span
                    className="clickable beta_label"
                    onClick={() =>
                        (location.href =
                            "https://srn4v-3aaaa-aaaar-qaftq-cai.icp0.io")
                    }
                >
                    BETA
                </span>
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
                        {token(
                            payment_token_locked,
                            paymentToken.decimals,
                            false,
                        )}{" "}
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
                    <span>FEE</span>
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
                        (entry) => entry[0] != paymentTokenId(),
                    ),
                ).map(([id, { symbol, logo }]) => (
                    <div
                        key={id}
                        className="row_container vcentered bottom_spaced x_large"
                    >
                        <div className="right_half_spaced vcentered">
                            {logo ? (
                                <img src={logo} width="20px" height="20px" />
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
                                const consent =
                                    "Listing on BEACON is fully permissionless. " +
                                    "There is no third party who can support you in help of any problems. " +
                                    "Moreover, your token can get delisted if it stops being traded for a long period of time " +
                                    "(you will be able to relist it again an any time later).";
                                if (!confirm(consent)) return;
                                const input =
                                    prompt("Enter the canister id:") || "";
                                if (!input) return;
                                const id = Principal.fromText(input);
                                if (!id) return;
                                location.href = `#/list/${id.toString()}`;
                            } catch (e) {
                                alert(e);
                            }
                        }}
                    >
                        LIST YOUR TOKEN NOW!
                    </button>
                </div>
                <br />
                <br />
                <br />
                <hr className="top_spaced bottom_spaced" />
                <br />
                <br />
                <div className="README bottom_spaced">
                    {renderMarkdown(readme)}
                </div>
            </div>
        </>
    );
};

function renderMarkdown(markdown: string): React.ReactNode[] {
    const lines = markdown.split("\n");
    const reactNodes: React.ReactNode[] = [];

    let listItems: React.ReactNode[] = [];
    let isOrderedList = false;

    lines.forEach((line) => {
        // Convert headers
        if (/^(#{1,6})\s+(.*)$/.test(line)) {
            const matches = line.match(/^(#{1,6})\s+(.*)$/);
            if (matches) {
                const level = matches[1].length;
                const content = matches[2];
                reactNodes.push(
                    React.createElement(
                        `h${level}`,
                        { key: reactNodes.length },
                        content,
                    ),
                );
            }
            return;
        }

        // Convert ordered lists
        if (/^\d+\.\s+(.*)$/.test(line)) {
            const content = line.replace(/^\d+\.\s+/, "");
            listItems.push(<li key={listItems.length}>{content}</li>);
            isOrderedList = true;
            return;
        }

        // Convert unordered lists
        if (/^\-\s+(.*)$/.test(line)) {
            const content = line.replace(/^\-\s+/, "");
            listItems.push(<li key={listItems.length}>{content}</li>);
            isOrderedList = false;
            return;
        }

        // Handle the end of a list
        if (
            listItems.length > 0 &&
            !/^\d+\.\s+/.test(line) &&
            !/^\*\s+/.test(line)
        ) {
            const ListTag = isOrderedList ? "ol" : "ul";
            reactNodes.push(
                <ListTag key={reactNodes.length}>{listItems}</ListTag>,
            );
            listItems = [];
        }

        // Convert bold and italics
        if (/\*\*(.*?)\*\*/.test(line)) {
            line = line.replace(
                /\*\*(.*?)\*\*/g,
                (_, content) => `<strong>${content}</strong>`,
            );
        }
        if (/\*(.*?)\*/.test(line)) {
            line = line.replace(
                /\*(.*?)\*/g,
                (_, content) => `<em>${content}</em>`,
            );
        }

        // Convert inline code
        if (/`([^`]+)`/.test(line)) {
            line = line.replace(
                /`([^`]+)`/g,
                (_, content) => `<code>${content}</code>`,
            );
        }

        // Convert links
        if (/\[([^\]]+)\]\(([^)]+)\)/.test(line)) {
            line = line.replace(
                /\[([^\]]+)\]\(([^)]+)\)/g,
                (_, text, url) => `<a href="${url}">${text}</a>`,
            );
        }

        // Push other lines as <p> elements
        if (line.trim()) {
            reactNodes.push(
                <p
                    key={reactNodes.length}
                    dangerouslySetInnerHTML={{ __html: line }}
                />,
            );
        }
    });

    // Handle any remaining list items
    if (listItems.length > 0) {
        const ListTag = isOrderedList ? "ol" : "ul";
        reactNodes.push(<ListTag key={reactNodes.length}>{listItems}</ListTag>);
    }

    return reactNodes;
}
