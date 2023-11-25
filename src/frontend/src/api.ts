import { Principal } from "@dfinity/principal";
import { HttpAgent, HttpAgentOptions, Identity, polling } from "@dfinity/agent";
import { mainnetMode } from "./common";

export type Backend = {
    query: <T>(
        methodName: string,
        arg0?: unknown,
        arg1?: unknown,
        arg2?: unknown,
        arg3?: unknown,
        arg4?: unknown,
    ) => Promise<T | null>;

    query_raw: (
        methodName: string,
        arg: ArrayBuffer,
    ) => Promise<ArrayBuffer | null>;

    call: <T>(
        methodName: string,
        arg0?: unknown,
        arg1?: unknown,
        arg2?: unknown,
        arg3?: unknown,
        arg4?: unknown,
        arg5?: unknown,
    ) => Promise<T | null>;
};

export const ApiGenerator = (
    defaultCanisterId: string,
    identity?: Identity,
): Backend => {
    const canisterId = Principal.fromText(defaultCanisterId);
    const options: HttpAgentOptions = { identity };
    if (mainnetMode) options.host = `https://${defaultCanisterId}.icp0.io`;
    const agent = new HttpAgent(options);
    if (!mainnetMode)
        agent.fetchRootKey().catch((err) => {
            console.warn(
                "Unable to fetch root key. Check to ensure that your local replica is running",
            );
            console.error(err);
        });

    const query_raw = async (
        methodName: string,
        arg = new ArrayBuffer(0),
    ): Promise<ArrayBuffer | null> => {
        let response = await agent.query(
            canisterId,
            { methodName, arg },
            identity,
        );
        if (response.status != "replied") {
            console.error(response);
            return null;
        }
        return response.reply.arg;
    };

    const query = async <T>(
        methodName: string,
        arg0?: unknown,
        arg1?: unknown,
        arg2?: unknown,
        arg3?: unknown,
        arg4?: unknown,
    ): Promise<T | null> => {
        let effParams = getEffParams([arg0, arg1, arg2, arg3, arg4]);
        const arg = Buffer.from(JSON.stringify(effParams));

        const response = await query_raw(methodName, arg);
        if (!response) {
            return null;
        }
        return JSON.parse(Buffer.from(response).toString("utf8"));
    };

    const call_raw = async (
        methodName: string,
        arg: ArrayBuffer,
    ): Promise<ArrayBuffer | null> => {
        let { response, requestId } = await agent.call(
            canisterId,
            { methodName, arg },
            identity,
        );
        if (!response.ok) {
            console.error(`Call error: ${response.statusText}`);
            return null;
        }
        return await polling.pollForResponse(
            agent,
            canisterId,
            requestId,
            polling.defaultStrategy(),
        );
    };

    const call = async <T>(
        methodName: string,
        arg0?: unknown,
        arg1?: unknown,
        arg2?: unknown,
        arg3?: unknown,
        arg4?: unknown,
        arg5?: unknown,
    ): Promise<T | null> => {
        const effParams = getEffParams([arg0, arg1, arg2, arg3, arg4, arg5]);
        const responseBytes = await call_raw(
            methodName,
            Buffer.from(JSON.stringify(effParams)),
        );
        if (!responseBytes || !responseBytes.byteLength) {
            return null;
        }
        return JSON.parse(Buffer.from(responseBytes).toString("utf8"));
    };

    return {
        query,
        query_raw,
        call,
    };
};

const getEffParams = <T>(args: T[]): T | T[] | null => {
    const values = args.filter((val) => typeof val != "undefined");
    if (values.length == 0) return null;
    if (values.length == 1) {
        return values[0];
    }
    return values;
};
