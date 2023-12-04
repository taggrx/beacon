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
