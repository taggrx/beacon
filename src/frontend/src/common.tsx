import * as React from "react";
import { Order } from "./types";
export const Error = ({ text }: { text: string }) => <h1>Error: {text}</h1>;

export const PAYMENT_TOKEN_ID = "ryjl3-tyaaa-aaaaa-aaaba-cai";

export const mainnetMode = process.env.NODE_ENV == "production";

export const II_URL = mainnetMode
    ? "https://identity.ic0.app"
    : "http://127.0.0.1:8080/?canisterId=qhbym-qaaaa-aaaaa-aaafq-cai";

export const II_DERIVATION_URL = mainnetMode
    ? `https://${process.env.CANISTER_ID}.icp0.io`
    : window.location.origin;

export const token = (amount: BigInt, decimals: number) => {
    let n = Number(amount);
    let base = Math.pow(10, decimals);
    let a = Math.floor(n / base);
    let b = n % base;
    return parseFloat(`${a}.${b}`);
};

export const tokenFee = (tokenId: string) =>
    BigInt(window.tokenData[tokenId].fee);

export const bigScreen = () => window.innerWidth >= 1024;

export const humanReadablePrice = (price: bigint, tokenId: string) =>
    BigInt(Number(price) * Math.pow(10, window.tokenData[tokenId].decimals));

export const orderId = (order: Order) =>
    order.owner.toString() +
    order.price.toString() +
    order.timestamp +
    order.amount +
    order.executed;

export const Button = ({
    onClick,
    classNameArg,
    styleArg = {},
    label,
    disabled,
}: {
    classNameArg?: string;
    onClick: () => Promise<void>;
    styleArg?: { [key: string]: string };
    label: string;
    disabled?: boolean;
}) => {
    const [loading, setLoading] = React.useState(false);
    const off = disabled || loading;
    if (off) styleArg.opacity = "0.5";
    else delete styleArg.opacity;
    return (
        <button
            style={styleArg}
            className={classNameArg}
            disabled={off}
            onClick={async () => {
                setLoading(true);
                await onClick();
                setLoading(false);
            }}
        >
            {loading ? <progress></progress> : label}
        </button>
    );
};

export const ConnectButton = ({}) => (
    <button
        className="active"
        style={{ background: "#6ac2c9", color: "#111111" }}
        onClick={() =>
            window.authClient.login({
                onSuccess: () => location.reload(),
                identityProvider: II_URL,
                maxTimeToLive: BigInt(30 * 24 * 3600000000000),
                derivationOrigin: II_DERIVATION_URL,
            })
        }
    >
        CONNECT
    </button>
);

export const CopyToClipboard = ({
    value,
    classNameArg,
    displayMap = (id) => id,
}: {
    value: string;
    classNameArg?: string;
    displayMap?: (arg: string) => string;
}) => {
    const [copied, setCopied] = React.useState(false);
    return (
        <span
            className={classNameArg}
            style={{ cursor: "pointer" }}
            title="Copy to clipboard"
            onClick={async () => {
                const cb = navigator.clipboard;
                await cb.writeText(value);
                setCopied(true);
            }}
        >
            <code>{displayMap(value)} </code>
            {copied ? (
                <>✅</>
            ) : (
                <>
                    [<span className="clickable">COPY</span>]
                </>
            )}
        </span>
    );
};
