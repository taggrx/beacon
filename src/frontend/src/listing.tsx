import { Principal } from "@dfinity/principal";
import * as React from "react";
import {
    Button,
    CopyToClipboard,
    PAYMENT_TOKEN_ID,
    token,
    tokenFee,
} from "./common";
import { Result } from "./types";

export const Listing = ({ id }: { id: string }) => {
    const [status, setStatus] = React.useState("");
    const amount = BigInt(Number(window.data.e8s_per_xdr) * 100);
    const price = <code>{token(amount, 8)} ICP</code>;
    return (
        <>
            <h1>
                Token <code>{id}</code> is not listed yet!
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
                            let deposit_result: any = await window.api.transfer(
                                Principal.fromText(PAYMENT_TOKEN_ID),
                                Principal.fromText(
                                    process.env.CANISTER_ID || "",
                                ),
                                window.principalId.toUint8Array(),
                                amount - tokenFee(PAYMENT_TOKEN_ID),
                            );
                            if (!deposit_result) {
                                setStatus("🔴 Call failed.");
                                return;
                            }
                            if ("Err" in deposit_result) {
                                console.error(deposit_result.Err);
                                setStatus(
                                    "🔴 Error: listing failed. Please check if you deposited enough funds!",
                                );
                                return;
                            }
                            setStatus("Deposited ICP to Beacon...");
                            const result = await window.api.call<Result<null>>(
                                "list_token",
                                id,
                            );
                            if (!result) {
                                setStatus("🔴 Call failed.");
                                return;
                            }
                            if ("Err" in result) {
                                setStatus(`🔴 Error: ${result.Err}`);
                                return;
                            }
                            location.reload();
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
