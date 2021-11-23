use super::*;
use stats::median;
use std::cmp::Ordering;
use std::collections::BinaryHeap;

#[derive(Debug)]
struct Needed {
    symbol: String,
    cash_delta: f32,
    percentage_delta: f32,
}

impl Needed {
    fn new(symbol: &str, cash_delta: f32, portfolio: &Portfolio) -> Self {
        let balanced_amount =
            portfolio.target.get(symbol).expect("missing target") * portfolio.total_value();
        let percentage_delta = if balanced_amount > 0.0 {
            cash_delta / balanced_amount
        } else {
            0.0
        };
        Needed {
            symbol: symbol.into(),
            cash_delta,
            percentage_delta,
        }
    }
}

impl Eq for Needed {}

impl PartialOrd for Needed {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.percentage_delta.partial_cmp(&other.percentage_delta)
    }
}

impl Ord for Needed {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl PartialEq for Needed {
    fn eq(&self, other: &Self) -> bool {
        self.percentage_delta == other.percentage_delta
    }
}

pub fn run_balancing(portfolio: Portfolio) -> Results {
    let total_value = portfolio.total_value();
    let allocations = c! { s => w * total_value, for (s, w) in portfolio.target.iter() };
    let total_shares = portfolio.total_shares();
    // Portfolio::validate has already checked for the necessary prices
    let prices = c! { &i.symbol => i.price, for i in portfolio.market.iter() };
    let yields = c! { &i.symbol => i.div_yield.unwrap_or(0.0), for i in portfolio.market.iter() };
    let cash_delta = c! { s => a - total_shares.get(s).unwrap_or(&0.0) * prices.get(s).unwrap(),
    for (s, a) in allocations };

    let mut shares_delta = c! { s => d / prices.get(s).expect("missing price"),
    for (s, d) in &cash_delta };

    let mut symbols_by_price = c![ (&i.symbol, i.price), for i in portfolio.market.iter() ];
    // price descending
    symbols_by_price.sort_by(|(_, a), (_, b)| (b.round() as i32).cmp(&(a.round() as i32)));
    let symbols_by_price: Vec<&String> = symbols_by_price.iter().map(|(s, _)| *s).collect();

    let median_yield = median(portfolio.market.iter().map(|i| i.div_yield.unwrap_or(0.0)));

    let mut accounts = portfolio.accounts.to_vec();
    accounts.sort_by(|a, b| b.tax_sheltered.cmp(&a.tax_sheltered)); // sheltered accounts first
    let taxable_first = {
        let mut a = accounts.clone();
        a.reverse();
        a
    };

    let mut results = Results::from_positions(&accounts);

    println!(
        "Accounts before action: {:?} with value {}",
        results.positions, total_value
    );
    println!("   disallowing sales: {:?}", &portfolio.no_sale_accounts);
    println!("Shares delta before action: {:?}", shares_delta);
    println!("Median yield={:?}, yields={:?}", median_yield, yields);

    // first sell shares we're overweight in
    for (sym, delta) in shares_delta.iter_mut() {
        if *delta > -1.0 {
            continue;
        }
        println!("overweight in {}, selling {}", sym, delta);
        let price = *prices.get(*sym).expect("unexpected missing price");

        for account in accounts.iter() {
            if !account.tax_sheltered && !portfolio.can_sell_taxed() {
                continue;
            }
            if portfolio.no_sale_accounts.contains(&account.name) {
                continue;
            }
            let acct_shares = results.transact(&account.name, sym, 0.0);
            // positive number of shares to sell
            let to_sell = if delta.abs().gt(&acct_shares) {
                acct_shares
            } else {
                delta.abs().floor()
            };
            if to_sell < 1.0 {
                continue;
            }

            if let Some(_) = results.buy_maybe(&account.name, sym, price, -1.0 * to_sell) {
                *delta += to_sell;
                println!(
                    "In acct={} sold {} x {}@{}, fc={:?}. Remaining delta={}",
                    account.name, to_sell, sym, price, &results.cash, delta
                );
            }
        }
    }

    println!(
        "Results after sale of overweight positions: r={:?}",
        results
    );

    // now prepare to buy shares, in the order in which they're most needed
    println!("cash delta before buys: {:?}", &cash_delta);
    let mut needed_funds = BinaryHeap::new();
    for (sym, value_needed) in cash_delta.into_iter() {
        needed_funds.push(Needed::new(sym, value_needed, &portfolio));
    }

    println!("needed heap before start: {:?}", needed_funds);

    loop {
        let next = match needed_funds.pop() {
            Some(n) => n,
            None => break,
        };
        let symbol = &next.symbol;
        let price = *prices.get(symbol).expect("unexpected missing price");
        let shares = next.cash_delta / price;
        if shares <= 0.0 {
            continue;
        }

        let mut bought = false;

        // if the fund is 'high yield', try to allocate into a tax-sheltered account
        let is_high_yield = match yields.get(symbol) {
            Some(div_yield) => match median_yield {
                Some(median) if *div_yield as f64 > median => true, // success!
                _ => false,
            },
            _ => false,
        };
        if is_high_yield {
            for account in accounts.iter().filter(|a| a.tax_sheltered) {
                if let Some(_) = results.buy_maybe(&account.name, symbol, price, 1.0) {
                    bought = true;
                    println!(
                        "high-yield: acct={}, bought {}@{}, fc={:?}, diff={:.2}%",
                        account.name,
                        symbol,
                        price,
                        results.cash,
                        next.percentage_delta * 100.0,
                    );
                    break;
                }
            }
        }
        // otherwise just try to put it into the first account it fits into
        if !bought {
            // don't check the tax-sheltered accounts again if we already have
            for account in taxable_first
                .iter()
                .filter(|a| !is_high_yield || !a.tax_sheltered)
            {
                if let Some(_) = results.buy_maybe(&account.name, symbol, price, 1.0) {
                    bought = true;
                    println!(
                        "acct={}, bought {}@{}, fc={:?}, diff={:.2}%",
                        account.name,
                        symbol,
                        price,
                        results.cash,
                        next.percentage_delta * 100.0,
                    );
                    break;
                }
            }
        }

        if bought {
            let new_needed = next.cash_delta - price;
            if new_needed > 0.0 {
                needed_funds.push(Needed::new(&next.symbol, new_needed, &portfolio));
            }
        }
    }

    // at this point we're close to our target allocations, this loop uses the spare cash
    // by buying additional shares one at a time wherever they fit
    loop {
        let mut bought = false;

        for sym in symbols_by_price.iter() {
            let price = *prices.get(*sym).expect("unexpected missing price");
            for account in accounts.iter() {
                if let Some(_) = results.buy_maybe(&account.name, sym, price, 1.0) {
                    bought = true;
                    println!(
                        "extra: acct={}, bought {}@{}, fc={:?}",
                        account.name, sym, price, results.cash
                    );
                    break; // we found an account to hold the extra share, move on to next fund
                }
            }
        }

        if !bought {
            break;
        }
    }

    results.calculate_percentages(&prices);
    println!("Results after balancing: {:?}", results);
    results
}

#[cfg(test)]
mod single_account {
    use super::test::check_allocation;
    use super::*;
    use spectral::prelude::*;
    use std::ops::IndexMut;

    #[test]
    fn check_empty_case() {
        assert_eq!(run_balancing(Portfolio::new()), Results::new());
    }

    fn build_portfolio() -> Portfolio {
        let mut p = Portfolio::new();
        let mut acct = Account::new("taxed");
        acct.cash = 10_000.0;
        p.accounts.push(acct);
        p.target.insert(String::from("A"), 0.5);
        p.target.insert(String::from("B"), 0.5);
        p.market.push(Investment::new("A", 10.0));
        p.market.push(Investment::new("B", 100.0));
        p
    }

    #[test]
    fn simple_balance() {
        let p = build_portfolio();
        assert_that(&p.validate()).is_none();
        assert_that(&p.total_shares()).is_empty();

        let r = run_balancing(p);

        assert_that(&r.total_cash).is_close_to(0.0, 0.1);
        check_shares(&r, "taxed", "A", 500.0);
        check_shares(&r, "taxed", "B", 50.0);
    }

    #[test]
    fn simple_extra_cash() {
        let mut p = build_portfolio();
        p.accounts.index_mut(0).cash += 5.0;

        let r = run_balancing(p);

        assert_that(&r.total_cash).is_close_to(5.0, 0.1);
        check_shares(&r, "taxed", "A", 500.0);
        check_shares(&r, "taxed", "B", 50.0);
        check_allocation(&r, "A", 0.499);
        check_allocation(&r, "B", 0.499);
        check_allocation(&r, "cash", 0.001);
    }

    #[test]
    fn simple_cash_neeed() {
        let mut p = build_portfolio();
        p.accounts.index_mut(0).cash = -5_000.0;
        p.accounts
            .index_mut(0)
            .positions
            .insert("A".to_string(), 500.0);
        p.accounts
            .index_mut(0)
            .positions
            .insert("A".to_string(), 500.0);
        p.accounts
            .index_mut(0)
            .positions
            .insert("B".to_string(), 50.0);

        let r = run_balancing(p);

        assert_that(&r.total_cash).is_close_to(0.0, 0.1);
        check_allocation(&r, "A", 0.499);
        check_allocation(&r, "B", 0.499);
        check_allocation(&r, "cash", 0.001);
    }

    pub fn check_shares(r: &Results, acct: &str, sym: &str, expected: f32) {
        let account = r.positions.get(acct).expect("missing account");
        let shares = account.get(sym);
        assert_that(shares.unwrap_or(&0.0)).is_close_to(expected, 0.1);
    }

    fn build_sale_needed_portfolio() -> Portfolio {
        let mut p = build_portfolio();
        {
            let taxed = p.accounts.index_mut(0);
            // 100% B and no cash, will need to sell half to buy A
            taxed.cash = 0.0;
            taxed.positions.insert(String::from("B"), 100.0);
        }
        p
    }

    #[test]
    fn test_with_sale() {
        let p = build_sale_needed_portfolio();

        let r = run_balancing(p);

        assert_that(&r.total_cash).is_close_to(0.0, 0.1);
        check_shares(&r, "taxed", "A", 500.0);
        check_shares(&r, "taxed", "B", 50.0);
    }

    #[test]
    fn no_fractional_sales() {
        let mut p = build_sale_needed_portfolio();
        p.target.insert(String::from("A"), 0.34);
        p.target.insert(String::from("B"), 0.66);

        let r = run_balancing(p);

        assert_that(&r.total_cash).is_close_to(0.0, 0.1);
        check_shares(&r, "taxed", "A", 330.0);
        check_shares(&r, "taxed", "B", 67.0);
    }

    #[test]
    fn no_taxed_sales_allowed() {
        let mut p = build_sale_needed_portfolio();
        p.no_taxed_sales = Some(true);

        let r = run_balancing(p);

        assert_that(&r.total_cash).is_close_to(0.0, 0.1);
        check_shares(&r, "taxed", "B", 100.0);
    }

    #[test]
    fn no_sales_allowed() {
        let mut p = build_sale_needed_portfolio();
        p.no_sale_accounts.insert(String::from("taxed"));

        let r = run_balancing(p);

        assert_that(&r.total_cash).is_close_to(0.0, 0.1);
        check_shares(&r, "taxed", "B", 100.0);
    }

    #[test]
    fn taxed_sales_allowed_in_sheltered_account() {
        let mut p = build_sale_needed_portfolio();
        p.no_taxed_sales = Some(true);
        {
            let ira = p.accounts.index_mut(0);
            ira.name = String::from("ira");
            ira.tax_sheltered = true;
        }

        let r = run_balancing(p);

        assert_that(&r.total_cash).is_close_to(0.0, 0.1);
        check_shares(&r, "ira", "A", 500.0);
        check_shares(&r, "ira", "B", 50.0);
        check_allocation(&r, "A", 0.5);
        check_allocation(&r, "B", 0.5);
        check_allocation(&r, "cash", 0.0);
    }

    #[test]
    fn minimize_spare_cash() {
        let mut p = build_portfolio();
        {
            let a = p.accounts.index_mut(0);
            a.cash = 507.0;
            p.target.insert(String::from("C"), 0.0);
            p.market.push(Investment::new("C", 1.0));
        }

        let r = run_balancing(p);

        // overweight in B shares, we buy one we don't need all of
        assert_that(&r.total_cash).is_close_to(0.0, 0.1);
        check_shares(&r, "taxed", "A", 20.0);
        check_shares(&r, "taxed", "B", 3.0);
        check_shares(&r, "taxed", "C", 7.0);
        check_allocation(&r, "A", 0.394);
        check_allocation(&r, "B", 0.592);
        check_allocation(&r, "C", 0.014);
    }

    #[test]
    fn buys_most_needed_first() {
        let mut p = Portfolio::new();
        let mut acct = Account::new("taxed");
        acct.cash = 20.0; // not enough cash to fully balance
        acct.positions.insert(String::from("A"), 55.0);
        acct.positions.insert(String::from("B"), 25.0);
        acct.positions.insert(String::from("C"), 0.0);
        p.accounts.push(acct);
        p.no_sale_accounts.insert(String::from("taxed"));
        p.target.insert(String::from("A"), 0.33);
        p.target.insert(String::from("B"), 0.33);
        p.target.insert(String::from("C"), 0.34);

        p.market.push(Investment::new("A", 1.0));
        p.market.push(Investment::new("B", 1.0));
        p.market.push(Investment::new("C", 1.0));

        assert_that(&p.total_value()).is_close_to(100.0, 0.1);

        let r = run_balancing(p);

        // total value is 100, so an even balance would be ~33 each
        // but we can't get to that because too much A and no sales allowed
        // make sure we don't buy any B because we need C more
        assert_that(&r.total_cash).is_close_to(0.0, 0.1);
        check_shares(&r, "taxed", "A", 55.0);
        check_shares(&r, "taxed", "B", 25.0);
        check_shares(&r, "taxed", "C", 20.0);
    }

    #[test]
    fn buys_percentage_needed() {
        let mut p = Portfolio::new();
        let mut acct = Account::new("taxed");
        acct.cash = 3.0; // not enough cash to fully balance
        acct.positions.insert(String::from("A"), 88.0);
        acct.positions.insert(String::from("B"), 7.0);
        acct.positions.insert(String::from("C"), 0.0);
        p.accounts.push(acct);
        p.no_sale_accounts.insert(String::from("taxed"));
        p.target.insert(String::from("A"), 0.90);
        p.target.insert(String::from("B"), 0.08);
        p.target.insert(String::from("C"), 0.02);

        p.market.push(Investment::new("A", 1.0));
        p.market.push(Investment::new("B", 1.0));
        p.market.push(Investment::new("C", 1.0));

        let r = run_balancing(p);

        // we need $2 more A, $1 more B and $2 C
        // so naively we'd buy A and C
        // but it's 100% more C and ~2% more A so instead we buy B
        assert_that(&r.total_cash).is_close_to(0.0, 0.1);
        check_shares(&r, "taxed", "A", 88.0);
        check_shares(&r, "taxed", "B", 8.0);
        check_shares(&r, "taxed", "C", 2.0);
    }
}

#[cfg(test)]
mod multiple_accounts {
    use super::single_account::check_shares;
    use super::test::check_allocation;
    use super::*;
    use spectral::prelude::*;
    use std::ops::IndexMut;

    fn build_multi_portfolio() -> Portfolio {
        let mut p = Portfolio::new();
        let mut taxed = Account::new("taxed");
        taxed.cash = 8_000.0;
        p.accounts.push(taxed);
        let mut ira = Account::new("ira");
        ira.cash = 2_000.0;
        ira.tax_sheltered = true;
        p.accounts.push(ira);
        p.target.insert(String::from("A"), 0.5);
        p.target.insert(String::from("B"), 0.5);
        p.market.push(Investment::new("A", 10.0));
        p.market.push(Investment::new("B", 100.0));
        p
    }

    #[test]
    fn test_simple_multi() {
        let p = build_multi_portfolio();
        assert_that(&p.total_value()).is_close_to(10_000.0, 1.0);

        let r = run_balancing(p);

        assert_that(&r.total_cash).is_close_to(0.0, 0.1);
        check_shares(&r, "taxed", "A", 400.0); // 400*$10 = $4k, 50%
        check_shares(&r, "ira", "A", 100.0);

        check_shares(&r, "taxed", "B", 40.0); // 40*$100 = $4k, 50%
        check_shares(&r, "ira", "B", 10.0);

        check_allocation(&r, "A", 0.5);
        check_allocation(&r, "B", 0.5);
        check_allocation(&r, "cash", 0.0);
    }

    #[test]
    fn sell_tax_sheltered_first() {
        let mut p = build_multi_portfolio();
        {
            let taxed = p.accounts.index_mut(0);
            taxed.cash = 0.0;
            taxed.positions.insert(String::from("A"), 500.0);
        }
        {
            let ira = p.accounts.index_mut(1);
            ira.cash = 3_000.0;
            ira.positions.insert(String::from("A"), 200.0);
        }

        let r = run_balancing(p);

        assert_that(&r.total_cash).is_close_to(0.0, 0.1);
        // we have the needed 50% of A in the taxed account, sell extra from the ira
        check_shares(&r, "taxed", "A", 500.0);
        check_shares(&r, "ira", "A", 0.0);

        check_shares(&r, "taxed", "B", 0.0);
        check_shares(&r, "ira", "B", 50.0);
    }

    #[test]
    fn high_yield_tax_sheltered() {
        let mut p = build_multi_portfolio();
        p.market.index_mut(0).div_yield = Some(0.04); // A
        p.market.index_mut(1).div_yield = Some(0.01); // B

        let r = run_balancing(p);

        assert_that(&r.total_cash).is_close_to(0.0, 0.1);
        check_shares(&r, "taxed", "A", 300.0);
        check_shares(&r, "ira", "A", 200.0);

        check_shares(&r, "taxed", "B", 50.0);
        check_shares(&r, "ira", "B", 0.0); // IRA ends up entirely holding high-yield

        check_allocation(&r, "A", 0.5);
        check_allocation(&r, "B", 0.5);
    }
}
