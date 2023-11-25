import * as React from "react";
import { createRoot } from "react-dom/client";
import { ApiGenerator } from "./api";

const api = ApiGenerator(process.env.CANISTER_ID || "", undefined);

const App = ({}) => {
    const [response, setResponse] = React.useState<string | null>(null);
    React.useEffect(() => {
        api.query<string>("greet", "world").then(setResponse);
    }, []);
    return <p>{response}</p>;
};

const domRoot = document.getElementById("app");

if (domRoot) {
    const root = createRoot(domRoot);
    root.render(<App />);
}
