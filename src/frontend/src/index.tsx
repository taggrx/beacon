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
        e8s_per_xdr: number;
    }
}

const root = createRoot(document.getElementById("app") as Element);

const App = () => {
    const [token] = parseHash();
    if (token) return root.render(<Token id={token} />);
    root.render(
        <div className="text_centered">
            <h1>AnyToken</h1>
            <h2>Decentralized Exchange</h2>
        </div>,
    );
};

import { AuthClient } from "@dfinity/auth-client";
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

        window.e8s_per_xdr = (await window.api.query<number>("params")) || -1;

        App();
    },
);
