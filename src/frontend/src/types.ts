import { Principal } from "@dfinity/principal";

export type BackendData = {
    listing_price_usd: number;
    fee: bigint;
    volume_day: bigint;
    trades_day: number;
    payment_token_locked: bigint;
    cycle_balance: number;
    heap_size: number;
    tokens_listed: number;
    active_traders: number;
    payment_token_id: Principal;
};

export type Result<T> =
    | {
          ["Ok"]: T;
      }
    | {
          ["Err"]: string;
      };

export type TokenData = {
    account: string;
    balance: bigint;
};

export type Metadata = {
    symbol: string;
    fee: bigint;
    decimals: number;
    logo: string;
};

export enum OrderType {
    Buy = "Buy",
    Sell = "Sell",
}

export type Order = {
    timestamp: number;
    owner: Principal;
    amount: bigint;
    price: bigint;
    decimals: number;
    executed: number;
};

export type OrderExecution =
    | {
          ["Filled"]: number;
      }
    | {
          ["FilledAndOrderCreated"]: number;
      };
