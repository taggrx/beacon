import * as React from "react";
import { Metadata, Result } from "./types";
import { Button, ConnectButton, Error, icp } from "./common";

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
    const { e8s_per_xdr, icp_account } = window.data;
    const price = <code>{icp(BigInt(Number(e8s_per_xdr) * 100), 8)} ICP</code>;
    return (
        <>
            <h1>
                Token <code>{id}</code> is not listed yet!
            </h1>
            <p>Listing on AnyToken is permissionless and costs {price}. </p>
            {window.principalId && (
                <>
                    <p>
                        If you want to continue, transfer {price} to this
                        account:
                    </p>
                    <blockquote>
                        <code>{icp_account}</code>{" "}
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
            {!window.principalId && <ConnectButton />}
        </>
    );
};
