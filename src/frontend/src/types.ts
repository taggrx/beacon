import { Principal } from "@dfinity/principal";

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
    owner: Principal;
    amount: bigint;
    price: bigint;
};
