import { Principal } from "@dfinity/principal";
import * as React from "react";
import {
    Button,
    CopyToClipboard,
    PAYMENT_TOKEN_ID,
    depositFromWallet,
    token,
    tokenBase,
} from "./common";

export const Listing = ({ tokenId }: { tokenId: string }) => {
    const [status, setStatus] = React.useState("");
    const amount = BigInt(
        window.data.listing_price_usd * tokenBase(PAYMENT_TOKEN_ID),
    );
    const { symbol, decimals } = window.tokenData[PAYMENT_TOKEN_ID];
    const price = (
        <code>
            {token(amount, decimals)} {symbol}
        </code>
    );
    return (
        <>
            <h1>
                <code>{tokenId}</code>
            </h1>
            <p>This token is not listed yet!</p>
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

                    {!status && (
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
                                setStatus(`DONE!`);
                            }}
                        />
                    )}
                </>
            )}
            {!window.principalId && (
                <p>To list the token, please connect with BEACON first.</p>
            )}
        </>
    );
};
