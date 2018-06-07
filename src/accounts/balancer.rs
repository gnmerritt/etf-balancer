use super::*;

pub fn run_balancing(portfolio: Portfolio) -> Results {
    let total_value = portfolio.total_value();
    let prices = c!{ &i.symbol => i.price, for i in portfolio.market.iter() };
    let allocations = c!{ s => w * total_value, for (s, w) in portfolio.target.iter() };
    let total_shares = portfolio.total_shares();
    // Portfolio::validate has already checked for the necessary prices
    let cash_delta = c!{ s => a - total_shares.get(s).unwrap_or(&0.0) * prices.get(s).unwrap(),
                         for (s, a) in allocations };

    let mut shares_delta = c!{ s => d / prices.get(s).expect("missing price"),
                               for (s, d) in cash_delta };

    let mut symbols_by_price = c![ (&i.symbol, i.price), for i in portfolio.market.iter() ];
    // price descending
    symbols_by_price.sort_by(|(_, a), (_, b)| (b.round() as i32).cmp(&(a.round() as i32)));
    let symbols_by_price: Vec<&String> = symbols_by_price.iter().map(|(s, _)| *s).collect();

    let mut accounts = portfolio.accounts.to_vec();
    accounts.sort_by(|a, b| b.tax_sheltered.cmp(&a.tax_sheltered)); // sheltered accounts first

    let mut results = Results::from_positions(&accounts);
    let mut free_cash = c!{ &a.name => a.cash, for a in accounts.iter() };

    println!("Accounts before action: {:?}", results.positions);
    println!("Shares delta before action: {:?}", shares_delta);
    println!("Free cash before action: {:?}", free_cash);

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
            let acct_shares = results.positions
                .get_mut(&account.name).expect("missing acct")
                .entry(sym.to_string())
                .or_insert(0.0);
            // positive number of shares to sell
            let to_sell = if delta.abs().gt(acct_shares) {
                *acct_shares
            } else {
                delta.abs()
            };
            if to_sell < 1.0 {
                continue;
            }

            let change = price * to_sell;
            free_cash.entry(&account.name)
                .and_modify(|c| *c += change)
                .or_insert(change);
            *acct_shares -= to_sell;
            *delta += to_sell;
            println!("In acct={} sold {} x {}@{}, fc={:?}. Remaining delta={}",
                account.name, to_sell, sym, price, free_cash, delta);
        }
    }

    println!("Results after sale of overweight positions: r={:?}, fc={:?}", results, free_cash);

    // buy some of each share we need more of
    loop {
        let mut none_left = true;

        for (sym, shares) in shares_delta.iter_mut() {
            let price = *prices.get(*sym).expect("unexpected missing price");

            for account in accounts.iter() {
                let cash = free_cash.entry(&account.name).or_insert(0.0);
                if *shares < 1.0 || price > *cash {
                    continue;
                }
                none_left = false;

                // TODO: factor this into the struct, I think
                *cash -= price;
                *shares -= 1.0;
                results.positions
                    .get_mut(&account.name).expect("missing acct")
                    .entry(sym.to_string())
                    .and_modify(|e| *e += 1.0)
                    .or_insert(1.0);
                println!("acct={}, bought {}@{}, fc={:?}", account.name, sym, price, cash);
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
                let cash = free_cash.entry(&account.name).or_insert(0.0);
                if price > *cash {
                    continue;
                }
                none_left = false;

                *cash -= price;
                results.positions
                    .get_mut(&account.name).expect("missing acct")
                    .entry(sym.to_string())
                    .and_modify(|e| *e += 1.0)
                    .or_insert(1.0);
                println!("extra: acct={}, bought {}@{}, fc={:?}", account.name, sym, price, cash);
            }
        }

        if none_left {
            break;
        }
    }

    results.cash = free_cash.iter().map(|(_, c)| c).sum();
    println!("Results after balancing: {:?}", results);
    results
}

#[cfg(test)]
mod single_account {
    use std::ops::IndexMut;
    use spectral::prelude::*;
    use super::*;

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

        assert_that(&r.cash).is_close_to(0.0, 0.1);
        check_shares(&r, "taxed", "A", 500.0);
        check_shares(&r, "taxed", "B", 50.0);
    }

    #[test]
    fn simple_extra_cash() {
        let mut p = build_portfolio();
        p.accounts.index_mut(0).cash += 5.0;

        let r = run_balancing(p);

        assert_that(&r.cash).is_close_to(5.0, 0.1);
        check_shares(&r, "taxed", "A", 500.0);
        check_shares(&r, "taxed", "B", 50.0);
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

        assert_that(&r.cash).is_close_to(0.0, 0.1);
        check_shares(&r, "taxed", "A", 500.0);
        check_shares(&r, "taxed", "B", 50.0);
    }

    #[test]
    fn no_taxed_sales_allowed() {
        let mut p = build_sale_needed_portfolio();
        p.no_taxed_sales = Some(true);

        let r = run_balancing(p);

        assert_that(&r.cash).is_close_to(0.0, 0.1);
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

        assert_that(&r.cash).is_close_to(0.0, 0.1);
        check_shares(&r, "ira", "A", 500.0);
        check_shares(&r, "ira", "B", 50.0);
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
        assert_that(&r.cash).is_close_to(0.0, 0.1);
        check_shares(&r, "taxed", "A", 30.0);
        check_shares(&r, "taxed", "B", 2.0);
    }
}

#[cfg(test)]
mod multiple_accounts {
    use std::ops::IndexMut;
    use spectral::prelude::*;
    use super::*;
    use super::single_account::check_shares;

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

        assert_that(&r.cash).is_close_to(0.0, 0.1);
        // TODO: build more reporting into results to check these
        check_shares(&r, "taxed", "A", 480.0); // 500*$10 = $5k, 50%
        check_shares(&r, "ira", "A", 20.0);

        check_shares(&r, "taxed", "B", 32.0); // 50*$100 = $5k, 50%
        check_shares(&r, "ira", "B", 18.0);
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

        assert_that(&r.cash).is_close_to(0.0, 0.1);
        // we have the needed 50% of A in the taxed account, sell extra from the ira
        check_shares(&r, "taxed", "A", 500.0);
        check_shares(&r, "ira", "A", 0.0);

        check_shares(&r, "taxed", "B", 0.0);
        check_shares(&r, "ira", "B", 50.0);
    }
}
