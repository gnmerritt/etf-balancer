use super::*;
use stats::median;

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
    for (s, d) in cash_delta };

    let mut symbols_by_price = c![ (&i.symbol, i.price), for i in portfolio.market.iter() ];
    // price descending
    symbols_by_price.sort_by(|(_, a), (_, b)| (b.round() as i32).cmp(&(a.round() as i32)));
    let symbols_by_price: Vec<&String> = symbols_by_price.iter().map(|(s, _)| *s).collect();

    let median_yield = median(portfolio.market.iter().map(|i| i.div_yield.unwrap_or(0.0)));

    let mut accounts = portfolio.accounts.to_vec();
    accounts.sort_by(|a, b| b.tax_sheltered.cmp(&a.tax_sheltered)); // sheltered accounts first

    let mut results = Results::from_positions(&accounts);

    println!("Accounts before action: {:?}", results.positions);
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

            match results.buy_maybe(&account.name, sym, price, -1.0 * to_sell) {
                Some(_) => {
                    *delta += to_sell;
                    println!(
                        "In acct={} sold {} x {}@{}, fc={:?}. Remaining delta={}",
                        account.name, to_sell, sym, price, &results.cash, delta
                    );
                }
                _ => (),
            }
        }
    }

    println!(
        "Results after sale of overweight positions: r={:?}",
        results
    );

    loop {
        let mut none_left = true;

        // first allocate new 'high yield' shares into tax-sheltered accounts
        for (sym, shares) in shares_delta.iter_mut() {
            if *shares < 1.0 {
                continue;
            }
            match yields.get(*sym) {
                Some(div_yield) => match median_yield {
                    Some(median) if *div_yield as f64 > median => (), // success!
                    _ => continue,
                },
                _ => continue,
            }
            // if we got here the fund is higher-than-median yield
            let price = *prices.get(*sym).expect("unexpected missing price");

            for account in accounts.iter() {
                if !account.tax_sheltered {
                    continue;
                }
                match results.buy_maybe(&account.name, sym, price, 1.0) {
                    Some(_) => {
                        none_left = false;
                        *shares -= 1.0;
                        println!(
                            "high-yield: acct={}, bought {}@{}, fc={:?}",
                            account.name, sym, price, results.cash
                        );
                    }
                    _ => (),
                }
            }
        }

        if !none_left {
            continue;
        }

        // buy some of each share we need more of
        for (sym, shares) in shares_delta.iter_mut() {
            if *shares < 1.0 {
                continue;
            }
            let price = *prices.get(*sym).expect("unexpected missing price");

            for account in accounts.iter() {
                match results.buy_maybe(&account.name, sym, price, 1.0) {
                    Some(_) => {
                        none_left = false;
                        *shares -= 1.0;
                        println!(
                            "acct={}, bought {}@{}, fc={:?}",
                            account.name, sym, price, results.cash
                        );
                    }
                    _ => (),
                }
            }
        }

        if !none_left {
            continue;
        }

        // at this point we're close to our target allocations, this loop uses the spare cash
        // by buying additional shares one at a time wherever they fit
        for sym in symbols_by_price.iter() {
            let price = *prices.get(*sym).expect("unexpected missing price");

            for account in accounts.iter() {
                match results.buy_maybe(&account.name, sym, price, 1.0) {
                    Some(_) => {
                        none_left = false;
                        println!(
                            "extra: acct={}, bought {}@{}, fc={:?}",
                            account.name, sym, price, results.cash
                        );
                        break; // we found an account to hold the extra share, move on to next fund
                    }
                    _ => (),
                }
            }
        }

        if none_left {
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
            a.cash = 500.0;
        }

        let r = run_balancing(p);

        // this is overweight in A shares since there was spare cash
        assert_that(&r.total_cash).is_close_to(0.0, 0.1);
        check_shares(&r, "taxed", "A", 30.0);
        check_shares(&r, "taxed", "B", 2.0);
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

        let r = run_balancing(p);

        assert_that(&r.total_cash).is_close_to(0.0, 0.1);
        check_shares(&r, "taxed", "A", 480.0); // 500*$10 = $5k, 50%
        check_shares(&r, "ira", "A", 20.0);

        check_shares(&r, "taxed", "B", 32.0); // 50*$100 = $5k, 50%
        check_shares(&r, "ira", "B", 18.0);

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
