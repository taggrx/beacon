import React from "react";
import {
    Button,
    CopyToClipboard,
    checkICPDeposit,
    withdrawICP,
} from "./src/common";

export const IcpWallet = ({}: {}) => {
    const [withdrawalAccount, setWithdrawalAccount] = React.useState("");
    const [depositStatus, setDepositStatus] = React.useState("");
    const [withdrawalStatus, setWithdrawalStatus] = React.useState("");
    if (!window.principalId) return <h1>Unauthenticated</h1>;
    return (
        <div className="column_container">
            <h1>ICP WALLET</h1>
            <h2>DEPOSIT</h2>
            <p>
                To deposit ICP for trading, transfer any amount of ICP to your
                BEACON account:
            </p>
            <p>
                <CopyToClipboard value={window.data.icp_account} />
            </p>
            <br />
            {depositStatus && <span>{depositStatus}</span>}
            {!depositStatus && (
                <Button
                    onClick={() => checkICPDeposit(setDepositStatus)}
                    label="CHECK DEPOSIT"
                />
            )}
            <br />
            <h2>WITHDRAW</h2>
            <p>Specify the ICP account:</p>
            <input
                type="text"
                onChange={(e) => setWithdrawalAccount(e.target.value)}
            />
            <br />
            {withdrawalStatus && <span>{withdrawalStatus}</span>}
            {!withdrawalStatus && (
                <Button
                    onClick={() =>
                        withdrawICP(withdrawalAccount, setWithdrawalStatus)
                    }
                    label="WITHDRAW"
                />
            )}
        </div>
    );
};
