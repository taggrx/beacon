import * as React from "react";
import { Metadata, Result } from "./types";
import { Error } from "./common";
import { Listing } from "./listing";

export const Token = ({ id }: { id: string }) => {
    const [metadata, setMetadata] = React.useState<Result<Metadata> | null>();
    const loadToken = async (id: string) => {
        const [metadata] = await Promise.all([
            await window.api.query<Result<Metadata>>("token", id),
        ]);
        setMetadata(metadata);
    };
    React.useEffect(() => {
        if (id) loadToken(id);
    }, []);

    if (!metadata) return <Error text="something went wrong." />;

    if ("Err" in metadata) {
        return <Listing id={id} />;
    }

    const { symbol, logo } = metadata.Ok;
    return (
        <>
            <h1 className="row_container vcentered">
                <img
                    height="50"
                    width="50"
                    src={logo}
                    className="align-middle"
                />
                <code>{symbol}</code>
            </h1>
        </>
    );
};
