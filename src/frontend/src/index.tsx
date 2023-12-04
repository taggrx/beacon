import { createRoot } from "react-dom/client";
import { Token } from "./token";
import { ApiGenerator, Backend } from "./api";
import { Principal } from "@dfinity/principal";

const parseHash = (): string[] => {
    const parts = window.location.hash.replace("#", "").split("/");
    parts.shift();
    return parts.map(decodeURI);
};

type Data = {
    e8s_per_xdr: bigint;
};

declare global {
    interface Window {
        api: Backend;
        principalId: Principal;
        authClient: AuthClient;
        data: Data;
        refreshBackendData: () => Promise<void>;
    }
}

const root = createRoot(document.getElementById("app") as Element);

const App = () => {
    const [param] = parseHash();

    let content = null;

    if (param == "icp") {
        {
            /* content = <IcpWallet />; */
        }
    } else {
        content = <Token id={param} />;
    }
    if (content)
        return root.render(
            <>
                <Header />
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
AuthClient.create({ idleOptions: { disableIdle: true } }).then(
    async (authClient) => {
        window.authClient = authClient;
        let identity;
        if (await authClient.isAuthenticated()) {
            identity = authClient.getIdentity();
            if (identity) window.principalId = identity.getPrincipal();
        }
        window.api = ApiGenerator(process.env.CANISTER_ID || "", identity);

        window.refreshBackendData = async () => {
            console.log("Fetching backend data...");
            const [data]: any = await Promise.all([
                await window.api.query<Data>("params"),
            ]);
            window.data = data;
            App();
        };

        await window.refreshBackendData();
        setTimeout(window.refreshBackendData, 10 * 60 * 1000);
        window.addEventListener("popstate", App);
    },
);
