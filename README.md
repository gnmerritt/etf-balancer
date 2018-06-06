[![Build Status](https://travis-ci.org/gnmerritt/etf-balancer.svg?branch=master)](https://travis-ci.org/gnmerritt/etf-balancer)

# etf-balancer

Try and optimally balance an account of Vanguard ETFs given a target allocation. Runs as an API for easy use from google sheets.

## Given info
```
symbols:   [world, mid cap, reit, 500, small cap, bond]
desired %: [25,    10,      5,    35,  10,        15]
current $: []
yield %:   [2.6,   1.3,     4.4,  1.7, 1.2,       2.6]
```

shares-per-account: []

free cash per account (roth, ira, taxed)

total funds = sum(free cash) + sum(invested)

delta $ per ETF: [desired - invested foreach ETF]

delta shares per ETF: [delta/share price foreach ETF]

## ideas for allocation
   * put higher yield ETFs into tax-advantaged accts
   * keep most rebalancing in tax-advantaged accts
      * this means some of each fund needs to be tax sheltered
   * do as little as possible (minimize # transactions from current state)

assign primary allocations:

```
15% roth    -> bond, 1/2 reit
30% ira     -> foreign, 1/2 reit
55% taxable -> us small, us mid, 500
```

10% of each account should mirror portfolio, to ease rebalancing
   * TODO: how to test this over time?

## Iterative algorithm

```
for each ETF:
   if we're overweight, sell shares
      - from a sheltered account first
      - TODO: when okay to sell from taxable account?

for each account:
    make sure that there's at least 10% * desired % of each ETF
     -only buy shares if there's: free cash && we need shares

for each primary allocation:
    buy shares in acct until one of
       - we don't need more
       - account has no more free cash

for each share we still need more of, sorted by yield:
   fill tax advantaged accounts with shares
   fill taxable accounts
```
