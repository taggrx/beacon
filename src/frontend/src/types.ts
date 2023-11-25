export type Result<T> =
    | {
          ["Ok"]: T;
      }
    | {
          ["Err"]: string;
      };

export type Metadata = {
    symbol: string;
    fee: number;
    decimals: number;
    logo: string;
};
