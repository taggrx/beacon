import * as React from "react";
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
    label,
}: {
    onClick: () => Promise<void>;
    label: string;
}) => {
    const [loading, setLoading] = React.useState(false);
    return (
        <button
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
