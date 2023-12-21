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
    fee: bigint;
};

declare global {
    interface Window {
        api: Backend;
        principalId: Principal;
        authClient: AuthClient;
        data: Data;
        tokenData: { [key: string]: Metadata };
        internalBalances: {
            [key: string]: [bigint, bigint];
        };
        balances: { [key: string]: bigint };
        refreshBackendData: () => Promise<void>;
    }
}

const root = createRoot(document.getElementById("app") as Element);

const App = () => {
    const [param] = parseHash();

    let content = <Landing />;

    if (typeof param == "string") {
        content = <Token tokenId={param} />;
    }
    if (content)
        return root.render(
            <>
                <Header />
                {content}
            </>,
        );
    root.render(<Landing />);
};

import { AuthClient } from "@dfinity/auth-client";
import { Header } from "./header";
import { Metadata } from "./types";
import { Landing } from "./landing";
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
            console.log("Fetching data...");
            const [data, tokenData, internalBalances]: any = await Promise.all([
                window.api.query<Data>("params"),
                window.api.query<{ [key: string]: Metadata }>("tokens"),
                window.api.query<{
                    [key: string]: [bigint, bigint];
                }>("token_balances"),
            ]);
            window.data = data;
            window.tokenData = tokenData;
            window.internalBalances = internalBalances;
            window.balances = {};
            if (window.principalId) {
                let results = await Promise.all(
                    Object.keys(window.tokenData).map(async (tokenId) => {
                        let balance = await window.api.account_balance(
                            Principal.fromText(tokenId),
                            window.principalId,
                        );
                        return [tokenId, balance];
                    }),
                );
                results.forEach(
                    ([tokenId, balance]: any) =>
                        (window.balances[tokenId] = balance),
                );
            }
            App();
        };

        await window.refreshBackendData();
        setTimeout(window.refreshBackendData, 10 * 60 * 1000);
        window.addEventListener("popstate", App);
    },
);
