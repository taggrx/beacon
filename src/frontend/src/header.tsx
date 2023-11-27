import { ConnectButton, icp } from "./common";

export const Header = ({ icpBalance }: { icpBalance: BigInt }) => (
    <header className="row_container vcentered">
        <h3
            className="logo max_width_col"
            onClick={() => (location.href = "/")}
        >
            BEACON
        </h3>
        {window.principalId && (
            <>
                <a href="#/icp">{icp(icpBalance, 8)}</a>
                <span className="left_half_spaced right_spaced">ICP</span>
            </>
        )}
        {window.principalId ? (
            <button
                onClick={() => {
                    window.authClient.logout();
                    location.reload();
                }}
            >
                LOG OUT
            </button>
        ) : (
            <ConnectButton />
        )}
    </header>
);
