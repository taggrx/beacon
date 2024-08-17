# README

BEACON is a DEFI primitive for the IC ecosystem. It's an **immutable**, autonomous order-book based exchange that acts as an escrow service for users trading ICRC1 tokens.

## User Experience

BEACON is designed to provide an optimal combination of simple user experience and resilience. To optimize transaction fees and ensure maximum resilience, BEACON separates asynchronous user actions (like depositing/withdrawal of funds to/from BEACON) from synchronous trading.

The synchronous trading allows BEACON to execute every trade on one global state (similar to Ethereum's atomic state transitions) and verify all balance change invariances after each trade so that any invariance violation can be safely reverted.

Moreover, while user's funds are under management of BEACON, the trading happens using "virtual balances" of users and hence no token transfer fees are charged.

## Fees

1. Listing a token costs `100` ckUSDC and is charged only once per listing.
2. The trading costs `0.2%` each trading side and is only charged when an order gets filled.

There are no other fees, implicit or explicit.

All charged fees are routed to the BEACON developers and will be used for developing of next versions of BEACON.

## Autonomy

In order to ensure a maximally autonomous operation, the BEACON canister exposes the heap size and the cycle balance on the landing page, so that anyone can verify the canister state and act accordingly (e.g. by abstaining from a trade or topping up the cycle balance).

Moreover, the cansiter regularly cleans up stale data:

-   Only last 10k logs are kept in memory,
-   Unfilled orders get closed after `90` days,
-   Filled orders get deleted from the archive after `180` days,
-   Tokens with no orders and no activity for `180` days, get delisted (they can be re-listed any time).

## Security

The code is [open sourced](https://github.com/taggrx/beacon) and was audited by competent and experienced parties. The canister is deployed to the Internet Computer and is fully immutable and cannot be modified without an IC protocol change approved by the NNS DAO.

## Future

Any new future BEACON version will be deployed as _new_ immutable canister so that users can always use older versions without adjusting their previous trust assumptions, until the new version withstands the test of time.

## Terms of Service

The BEACON canister is a fully autonomous, immutable primitive that operates independently without any centralized control or modification. Users acknowledge that there are no guarantees or liability from developers regarding the canister's operation or outcomes resulting from its use.

By interacting with BEACON, users understand that they assume full responsibility for their actions and must accept the risk of total loss of their funds due to potential bugs, unexpected behavior, or any other unforeseen circumstances.

Any interaction with the BEACON canister signifies the user's acceptance of these terms and conditions.
