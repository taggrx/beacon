import { createRoot } from "react-dom/client";
import { Token } from "./token";
import { ApiGenerator, Backend } from "./api";

const parseHash = (): string[] => {
    const parts = window.location.hash.replace("#", "").split("/");
    parts.shift();
    return parts.map(decodeURI);
};

declare global {
    interface Window {
        api: Backend;
        principalId: string;
        authClient: AuthClient;
        data: {
            e8s_per_xdr: BigInt;
            icp_balance: BigInt;
            icp_account: string;
        };
        refreshBackendData: () => Promise<void>;
    }
}

const root = createRoot(document.getElementById("app") as Element);

const App = () => {
    const [param] = parseHash();

    let content = null;

    if (param == "icp") {
        content = <IcpWallet />;
    } else {
        content = <Token id={param} />;
    }
    if (content)
        return root.render(
            <>
                <Header icpBalance={window.data.icp_balance} />
                {content}
            </>,
        );
    root.render(
        <div className="text_centered">
            <h1 className="logo">BEACON</h1>
            <h2>
                <s>Immutable</s> Order-Book Based Exchange
            </h2>
            - daily total volume - canister balance - most popular tokens
        </div>,
    );
};

import { AuthClient } from "@dfinity/auth-client";
import { Header } from "./header";
import { IcpWallet } from "../icp_wallet";
AuthClient.create({ idleOptions: { disableIdle: true } }).then(
    async (authClient) => {
        window.authClient = authClient;
        let identity;
        if (await authClient.isAuthenticated()) {
            identity = authClient.getIdentity();
            if (identity)
                window.principalId = identity.getPrincipal().toString();
        }
        window.api = ApiGenerator(process.env.CANISTER_ID || "", identity);

        window.refreshBackendData = async () => {
            console.log("Fetching backend data...");
            window.data = await window.api.query<any>("params");
            App();
        };

        await window.refreshBackendData();
        setTimeout(window.refreshBackendData, 10 * 60 * 1000);
        window.addEventListener("popstate", App);
    },
);
