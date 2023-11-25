import * as React from "react";
import { Metadata, Result } from "./types";
import { Button, Error, II_DERIVATION_URL, II_URL, icp } from "./common";

export const Token = ({ id }: { id: string }) => {
    const [metadata, setMetadata] = React.useState<Result<Metadata> | null>();
    const loadToken = async (id: string) => {
        const result = await window.api.query<Result<Metadata>>("token", id);
        setMetadata(result);
    };
    React.useEffect(() => {
        loadToken(id);
    }, []);

    if (!metadata) return <Error text="something went wrong." />;

    if ("Err" in metadata) {
        return <Listing id={id} />;
    }

    const { symbol, fee, decimals, logo } = metadata.Ok;
    return (
        <>
            <h1 className="aligned">
                <img
                    height="50"
                    width="50"
                    src={logo}
                    className="align-middle"
                />
                <code>{symbol}</code>
            </h1>
            <h3>
                Decimals: <code>{decimals}</code> &middot; Fee:{" "}
                <code>{fee}</code>
            </h3>
        </>
    );
};

export const Listing = ({ id }: { id: string }) => {
    const [subaccount, setSubaccount] = React.useState("");
    const loadData = async () => {
        const result = await window.api.query<string>("subaccount");
        setSubaccount(result || "");
    };
    React.useEffect(() => {
        loadData();
    }, []);

    const price = <code>{icp(BigInt(window.e8s_per_xdr * 100), 8)} ICP</code>;
    return (
        <>
            <h1>
                Token <code>{id}</code> is not listed yet!
            </h1>
            <p>Listing on AnyToken is permissionless and costs {price}. </p>
            {window.principalId && subaccount && (
                <>
                    <p>
                        If you want to continue, transfer {price} to this
                        account:
                    </p>
                    <blockquote>
                        <code>{subaccount}</code>{" "}
                    </blockquote>

                    <Button
                        label="LIST TOKEN"
                        onClick={async () => {
                            let result = await window.api.call<Result<null>>(
                                "list_token",
                                id,
                            );
                            if (!result) {
                                alert("Call failed.");
                                return;
                            }
                            if ("Err" in result) {
                                alert(`Error: ${result.Err}`);
                                return;
                            }
                            location.reload();
                        }}
                    />
                </>
            )}
            {!window.principalId && (
                <>
                    <button
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
                </>
            )}
        </>
    );
};
