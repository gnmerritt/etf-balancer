use super::*;

pub fn run_balancing(portfolio: Portfolio) -> Results {
    let total_value = portfolio.total_value();
    let prices = c!{ i.symbol.clone() => i.price, for i in portfolio.market.iter() };
    let allocations = c!{ s => w * total_value, for (s, w) in portfolio.target.iter() };
    let total_shares = portfolio.total_shares();
    // Portfolio::validate has already checked for the necessary prices
    let cash_delta = c!{ s => a - total_shares.get(s).unwrap_or(&0.0) * prices.get(s).unwrap(),
                         for (s, a) in allocations };

    let mut shares_delta = c!{ s => d / prices.get(s).unwrap(), for (s, d) in cash_delta };

    let mut results = Results::new();
    results.cash = portfolio.accounts.iter().map(|a| a.cash).sum();

    for a in &portfolio.accounts {
        results.positions.insert(a.name.clone(), a.positions.clone());
    }

    println!("Accounts before action: {:?}", results);
    println!("Shares delta before action: {:?}", shares_delta);

    let account = "taxed"; // TODO: remove

    // first sell shares we're overweight in
    for (sym, delta) in shares_delta.iter().filter(|(_, &d)| d < -1.0) {
        println!("overweight in {}, selling {}", sym, delta);
        let price = *prices.get(*sym).unwrap();
        results.cash += price * delta.abs();
        results.positions
            .get_mut(account).expect("missing acct")
            .entry(sym.to_string())
            .and_modify(|e| *e -= delta.abs());
        println!("Sold {} x {}@{}, fc={}", delta.abs(), sym, price, results.cash);
    }

    println!("Results after sale of overweight positions: {:?}", results);

    // buy some of each share we need more of
    while results.cash > 0.0 && !shares_delta.is_empty() {
        let mut none_left = true;

        for (sym, shares) in shares_delta.iter_mut() {
            let price = *prices.get(*sym).unwrap();
            if *shares < 1.0 || price > results.cash {
                continue;
            }
            none_left = false;

            // TODO: factor this into the struct, I think
            results.cash -= price;
            *shares -= 1.0;
            results.positions
                .get_mut(account).expect("missing acct")
                .entry(sym.to_string())
                .and_modify(|e| *e += 1.0)
                .or_insert(1.0);
            println!("Bought {}@{}, fc={}", sym, price, results.cash);
        }

        if none_left {
            break;
        }
    }

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

    fn check_shares(r: &Results, acct: &str, sym: &str, expected: f32) {
        let account = r.positions.get(acct).expect("missing account");
        let shares = account.get(sym);
        assert_that(shares.unwrap_or(&0.0)).is_close_to(expected, 0.1);
    }

    #[test]
    fn test_with_sale() {
        let mut p = build_portfolio();
        {
            let taxed = p.accounts.index_mut(0);
            // 100% B and no cash, will need to sell half to buy A
            taxed.cash = 0.0;
            taxed.positions.insert(String::from("B"), 100.0);
        }

        let r = run_balancing(p);

        assert_that(&r.cash).is_close_to(0.0, 0.1);
        check_shares(&r, "taxed", "A", 500.0);
        check_shares(&r, "taxed", "B", 50.0);
    }
}

#[cfg(test)]
mod multiple_accounts {
    // TODO
}
