import { ConnectButton, icp } from "./common";

export const Header = ({}) => (
    <header className="row_container vcentered">
        <h3 className="logo max_width_col">BEACON</h3>
        <a href="#/icp">{icp(window.data.icp_balance, 8)}</a>
        <span className="left_half_spaced right_half_spaced">ICP</span>
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
