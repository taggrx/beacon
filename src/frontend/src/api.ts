import { IDL, JsonValue } from "@dfinity/candid";
import { Principal } from "@dfinity/principal";
import { HttpAgent, HttpAgentOptions, Identity, polling } from "@dfinity/agent";
import { mainnetMode } from "./common";
import { OrderType } from "./types";

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
        canisterId: Principal,
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

    account_balance: (token: Principal, owner: Principal) => Promise<bigint>;

    orders: (tokenId: Principal, orderType: OrderType) => Promise<JsonValue>;

    list_token: (tokenId: Principal) => Promise<JsonValue>;

    deposit_liquidity: (tokenId: Principal) => Promise<JsonValue>;

    close_order: (
        tokenId: Principal,
        order_type: OrderType,
        amount: bigint,
        price: bigint,
        timestamp: number,
    ) => Promise<void>;

    trade: (
        tokenId: Principal,
        amount: bigint,
        price: bigint,
        orderType: OrderType,
    ) => Promise<JsonValue>;

    withdraw: (tokenId: Principal) => Promise<JsonValue>;

    transfer: (
        tokenId: Principal,
        recipient: Principal,
        subaccount: Uint8Array,
        amount: bigint,
        fee: bigint,
    ) => Promise<JsonValue>;
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
        canisterId: Principal,
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

        const response = await query_raw(canisterId, methodName, arg);
        if (!response) {
            return null;
        }
        return JSON.parse(Buffer.from(response).toString("utf8"));
    };

    const call_raw = async (
        canisterId: Principal,
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
            canisterId,
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
        account_balance: async (
            tokenId: Principal,
            principal: Principal,
        ): Promise<bigint> => {
            const arg = IDL.encode(
                [IDL.Record({ owner: IDL.Principal })],
                [{ owner: principal }],
            );
            const response: any = await query_raw(
                tokenId,
                "icrc1_balance_of",
                arg,
            );
            if (!response) return BigInt(0);
            return IDL.decode([IDL.Nat], response)[0] as unknown as bigint;
        },

        orders: async (
            tokenId: Principal,
            orderType: OrderType,
        ): Promise<JsonValue> => {
            const arg = IDL.encode(
                [IDL.Principal, IDL.Variant({ Buy: IDL.Null, Sell: IDL.Null })],
                [tokenId, { [orderType.toString()]: null }],
            );
            const response = await query_raw(canisterId, "orders", arg);

            return decode(
                response,
                IDL.Vec(
                    IDL.Record({
                        owner: IDL.Principal,
                        amount: IDL.Nat,
                        price: IDL.Nat,
                        decimals: IDL.Nat32,
                        executed: IDL.Nat64,
                        timestamp: IDL.Nat64,
                    }),
                ),
            );
        },

        close_order: async (
            tokenId: Principal,
            orderType: OrderType,
            amount: bigint,
            price: bigint,
            timestamp: number,
        ): Promise<void> => {
            const arg = IDL.encode(
                [
                    IDL.Principal,
                    IDL.Variant({ Buy: IDL.Null, Sell: IDL.Null }),
                    IDL.Nat,
                    IDL.Nat,
                    IDL.Nat64,
                ],
                [
                    tokenId,
                    { [orderType.toString()]: null },
                    amount,
                    price,
                    timestamp,
                ],
            );
            const response = await call_raw(canisterId, "close_order", arg);

            decode(response);
        },

        list_token: async (tokenId: Principal): Promise<JsonValue> => {
            const arg = IDL.encode([IDL.Principal], [tokenId]);
            const response = await call_raw(canisterId, "list_token", arg);
            return decode(
                response,
                IDL.Variant({
                    Ok: IDL.Null,
                    Err: IDL.Text,
                }),
            );
        },

        deposit_liquidity: async (tokenId: Principal): Promise<JsonValue> => {
            const arg = IDL.encode([IDL.Principal], [tokenId]);
            const response = await call_raw(
                canisterId,
                "deposit_liquidity",
                arg,
            );
            return decode(
                response,
                IDL.Variant({
                    Ok: IDL.Null,
                    Err: IDL.Text,
                }),
            );
        },

        trade: async (
            tokenId: Principal,
            amount: bigint,
            price: bigint,
            orderType: OrderType,
        ): Promise<JsonValue> => {
            const arg = IDL.encode(
                [
                    IDL.Principal,
                    IDL.Nat,
                    IDL.Nat,
                    IDL.Variant({ Buy: IDL.Null, Sell: IDL.Null }),
                ],
                [tokenId, amount, price, { [orderType.toString()]: null }],
            );
            const response = await call_raw(canisterId, "trade", arg);
            return decode(response, IDL.Vec(IDL.Tuple(IDL.Nat, IDL.Bool)));
        },

        withdraw: async (tokenId: Principal) => {
            const arg = IDL.encode([IDL.Principal], [tokenId]);
            const response = await call_raw(canisterId, "withdraw", arg);
            return decode(
                response,
                IDL.Variant({ Ok: IDL.Nat, Err: IDL.Unknown }),
            );
        },

        transfer: async (
            tokenId: Principal,
            recipient: Principal,
            subaccount: Uint8Array,
            amount: bigint,
            fee: bigint,
        ) => {
            let resized = new Uint8Array(32);
            resized.set(Uint8Array.from(subaccount).subarray(0, 32));
            const to = {
                owner: recipient,
                subaccount: [resized],
            };

            const arg = IDL.encode(
                [
                    IDL.Record({
                        to: IDL.Record({
                            owner: IDL.Principal,
                            subaccount: IDL.Opt(IDL.Vec(IDL.Nat8)),
                        }),
                        amount: IDL.Nat,
                    }),
                ],
                [
                    {
                        to,
                        amount,
                        fee: [fee],
                    },
                ],
            );
            const response = await call_raw(tokenId, "icrc1_transfer", arg);
            return decode(
                response,
                IDL.Variant({ Ok: IDL.Nat, Err: IDL.Unknown }),
            );
        },
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

const decode = (result: any, type?: any) => {
    if (!result) {
        throw new Error("Call failed");
    }
    if ("Err" in result) {
        throw new Error(`Error: ${result.Err}`);
    }
    if (!type) return {};
    return IDL.decode([type], result)[0];
};
