import * as React from "react";
import { Principal } from "@dfinity/principal";
import { Order } from "./types";
export const Error = ({ text }: { text: string }) => <h1>Error: {text}</h1>;

export const PAYMENT_TOKEN_ID = "xevnm-gaaaa-aaaar-qafnq-cai";

export const mainnetMode = process.env.NODE_ENV == "production";

export const II_URL = mainnetMode
    ? "https://identity.ic0.app"
    : "http://127.0.0.1:8080/?canisterId=qhbym-qaaaa-aaaaa-aaafq-cai";

export const II_DERIVATION_URL = mainnetMode
    ? `https://${process.env.CANISTER_ID}.icp0.io`
    : window.location.origin;

export const token = (
    amount: BigInt | number,
    decimals: number,
    showDecimals: boolean = true,
) => {
    const n = Number(amount);
    const base = Math.pow(10, decimals);
    const a = Math.floor(n / base);
    if (!showDecimals) return a.toLocaleString();
    let b = `${n % base}`;
    while (b.length < decimals) b = "0" + b;
    return parseFloat(`${a}.${b}`);
};

export const depositFromWallet = async (
    tokenId: string,
    statusCallback: (arg: string) => void,
) => {
    const balance = BigInt(window.balances[tokenId]);
    if (balance <= tokenFee(tokenId)) {
        return;
    }
    const tokenData = window.tokenData[tokenId];
    if (balance > 0) {
        statusCallback(
            `TRANSFERRING ${token(balance, tokenData.decimals)} ${
                tokenData.symbol
            } TO BEACON...`,
        );
        let fee = BigInt(tokenFee(tokenId));
        const result: any = await window.api.transfer(
            Principal.fromText(tokenId),
            Principal.from(process.env.CANISTER_ID),
            window.principalId.toUint8Array(),
            balance - fee,
            fee,
        );
        if ("Err" in result) {
            console.error(result.Err);
            statusCallback("🔴 TRANSFER TO BECAON FAILED.");
            return;
        }
        statusCallback("DEPOSITING TRANSFERRED FUNDS...");
        const deposit_result: any = await window.api.deposit_liquidity(
            Principal.fromText(tokenId),
        );
        if ("Err" in deposit_result) {
            console.error(result.Err);
            statusCallback("🔴 DEPOSIT FAILED.");
            return;
        }
        await window.refreshBackendData();
    }
};

export const TokenLogo = ({}) => (
    <span
        style={{
            width: "20px",
            color: "transparent",
            fontSize: "80%",
            textShadow: "0 0 0 #cbcbbf",
        }}
    >
        💎
    </span>
);

export const tokenBase = (tokenId: string) =>
    Math.pow(10, window.tokenData[tokenId].decimals);

export const tokenFee = (tokenId: string) => window.tokenData[tokenId].fee;

export const bigScreen = () => window.innerWidth >= 1024;

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
    return (
        <button
            style={styleArg}
            className={classNameArg + " " + (off ? "inactive" : "")}
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

export const ConnectButton = ({ large }: { large?: boolean }) => (
    <button
        className="active"
        style={{
            fontSize: large ? "large" : undefined,
            background: "#4aa2a9",
            color: "#111111",
            minWidth: large ? (bigScreen() ? "30%" : "100%") : undefined,
        }}
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

export const timeAgo = (
    originalTimestamp: BigInt | number,
    absolute?: boolean,
    format: "short" | "long" = "short",
) => {
    const timestamp = Number(originalTimestamp) / 1000000;
    const diff = Number(new Date()) - timestamp;
    const minute = 60 * 1000;
    const hour = minute * 60;
    const day = hour * 24;
    switch (true) {
        case !absolute && diff < minute:
            const seconds = Math.round(diff / 1000);
            return `${seconds}s ago`;
        case !absolute && diff < hour:
            return Math.round(diff / minute) + "m ago";
        case !absolute && diff < day:
            return Math.round(diff / hour) + "h ago";
        case diff < 90 * day:
            return `${new Intl.DateTimeFormat("default", {
                month: format,
                day: "numeric",
            }).format(timestamp)}`;
        default:
            return `${new Intl.DateTimeFormat("default", {
                year: "2-digit",
                month: format,
                day: "numeric",
            }).format(timestamp)}`;
    }
};
