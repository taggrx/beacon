import { Principal } from "@dfinity/principal";
import * as React from "react";
import { CopyToClipboard } from "./common";

export const Logs = ({}) => {
    const [logs, setLogs] = React.useState<[number, string][]>([]);

    const loadData = async () => {
        const logs = await window.api.query<[number, string][]>("logs");
        if (logs) setLogs(logs);
    };

    React.useEffect(() => {
        loadData();
    }, []);

    return (
        <>
            <h1>LOGS</h1>
            <ul>
                {logs.map(([id, log]) => (
                    <li key={id}>
                        <code>{id}</code>:{" "}
                        {render(log).reduce(
                            (acc, e) => (
                                <>
                                    {acc} {e}
                                </>
                            ),
                            <></>,
                        )}
                    </li>
                ))}
            </ul>
        </>
    );
};

const render = (log: string): JSX.Element[] =>
    log.split(" ").map((part: string) => {
        if (!isNaN(Number(part))) {
            return <code>{Number(part).toLocaleString()}</code>;
        }
        try {
            Principal.fromText(part);
            return (
                <CopyToClipboard
                    value={part}
                    displayMap={(e) => e.split("-")[0]}
                />
            );
        } catch (e) {}
        return <>{part}</>;
    });
