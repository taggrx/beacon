import { Principal } from "@dfinity/principal";
import * as React from "react";
import {
    Button,
    CopyToClipboard,
    PAYMENT_TOKEN_ID,
    depositFromWallet,
    token,
} from "./common";

export const Listing = ({ tokenId }: { tokenId: string }) => {
    const [status, setStatus] = React.useState("");
    const amount = BigInt(Number(window.data.e8s_per_xdr) * 100);
    const price = <code>{token(amount, 8)} ICP</code>;
    return (
        <>
            <h1>
                Token <code>{tokenId}</code> is not listed yet!
            </h1>
            <p>Listing on BEACON:</p>
            <ul>
                <li>works only with ICRC1 tokens,</li>
                <li>is fully permissionless,</li>
                <li>costs {price}.</li>
            </ul>
            {window.principalId && (
                <>
                    <p>
                        If you want to continue, transfer {price} to your BEACON
                        wallet account:
                    </p>
                    <blockquote>
                        <CopyToClipboard
                            value={window.principalId.toString()}
                        />
                    </blockquote>
                    <br />

                    {status && <p>{status}</p>}

                    <Button
                        label="LIST TOKEN"
                        onClick={async () => {
                            await depositFromWallet(
                                PAYMENT_TOKEN_ID,
                                setStatus,
                            );

                            setStatus("LISTING THE TOKEN...");
                            const result: any = await window.api.list_token(
                                Principal.from(tokenId),
                            );
                            if (!result) {
                                setStatus("🔴 Call failed.");
                                return;
                            }
                            if ("Err" in result) {
                                setStatus(`🔴 Error: ${result.Err}`);
                                return;
                            }
                            location.href = `#/${tokenId}`;
                        }}
                    />
                </>
            )}
            {!window.principalId && (
                <p>To list the token, please connect with BEACON first.</p>
            )}
        </>
    );
};
