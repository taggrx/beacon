import * as React from "react";
import { Result } from "./types";
export const Error = ({ text }: { text: string }) => <h1>Error: {text}</h1>;

export const mainnetMode = process.env.NODE_ENV == "production";

export const II_URL = mainnetMode
    ? "https://identity.ic0.app"
    : "http://127.0.0.1:8080/?canisterId=qhbym-qaaaa-aaaaa-aaafq-cai";

export const II_DERIVATION_URL = mainnetMode
    ? `https://${process.env.CANISTER_ID}.icp0.io`
    : window.location.origin;

export const icp = (e8s: BigInt, decimals: number = 2) => {
    let n = Number(e8s);
    let base = Math.pow(10, 8);
    let v = n / base;
    return (decimals ? v : Math.floor(v)).toLocaleString(undefined, {
        minimumFractionDigits: decimals,
    });
};

export const Button = ({
    onClick,
    classNameArg,
    label,
}: {
    classNameArg?: string;
    onClick: () => Promise<void>;
    label: string;
}) => {
    const [loading, setLoading] = React.useState(false);
    return (
        <button
            className={classNameArg}
            disabled={loading}
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

export const CopyToClipboard = ({ value }: { value: string }) => {
    const [copied, setCopied] = React.useState(false);
    return (
        <code
            style={{ cursor: "pointer" }}
            title="Copy to clipboard"
            onClick={async () => {
                const cb = navigator.clipboard;
                await cb.writeText(value);
                setCopied(true);
            }}
        >
            {value}{" "}
            {copied ? (
                <>[copied!]</>
            ) : (
                <>
                    [<span className="clickable">copy</span>]
                </>
            )}
        </code>
    );
};

export const checkICPDeposit = async (
    statusCallback: (arg: string) => void,
) => {
    let result = await window.api.call<Result<BigInt>>("check_icp_deposit");
    if (!result) return;
    if ("Ok" in result)
        statusCallback(`✅ Funds deposited: ${icp(result.Ok, 8)} ICP!`);
    else if ("Err" in result) statusCallback(`🔴 Error: ${result.Err}`);
    await window.refreshBackendData();
};

export const withdrawICP = async (
    withdrawalAccount: string,
    statusCallback: (arg: string) => void,
) => {
    let result = await window.api.call<Result<BigInt>>(
        "withdraw_icp",
        hexToBytes(withdrawalAccount),
    );
    if (!result) return;
    if ("Ok" in result)
        statusCallback(`✅ Funds withdrawn: ${icp(result.Ok, 8)} ICP`);
    else if ("Err" in result) statusCallback(`🔴 Error: ${result.Err}`);
    await window.refreshBackendData();
};

function hexToBytes(hex: string) {
    let bytes = [];
    for (let i = 0; i < hex.length; i += 2) {
        bytes.push(parseInt(hex.substr(i, 2), 16));
    }
    return bytes;
}
