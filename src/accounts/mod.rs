pub mod balancer;

use std::collections::{HashMap, HashSet};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Portfolio {
    target: HashMap<String, f32>,
    accounts: Vec<Account>,
    market: Vec<Investment>,
    no_taxed_sales: Option<bool>, // defaults to allowing sales
}

impl Portfolio {
    pub fn new() -> Self {
        Portfolio {
            target: HashMap::new(), accounts: vec!(), market: vec!(), no_taxed_sales: None
        }
    }

    pub fn validate(&self) -> Option<&'static str> {
        // make sure the requested allocations add up to 1 (100%)
        let sum: f32 = self.target.iter().map(|(_, p)| p).sum();
        if (sum - 1.0).abs() > 0.01 {
            return Some("Allocations must add up to 1.0")
        }
        // make sure we were given price info for all allocated and owned stocks
        let prices: HashSet<&String> = self.market.iter().map(|i| &i.symbol).collect();
        let shares = self.total_shares();
        let missing_owned = shares.iter().map(|(s, _)| s).filter(|s| !prices.contains(s)).count();
        let missing_allocated = self.target.iter().map(|(s, _)| s).filter(|s| !prices.contains(s)).count();
        if missing_allocated > 0 || missing_owned > 0 {
            return Some("Missing prices for some investments") // TODO: format this?
        }

        None
    }

    fn total_value(&self) -> f32 {
        self.accounts.iter().map(|a| a.value(&self.market)).sum::<f32>()
    }

    fn total_shares(&self) -> HashMap<String, f32> {
        let mut tot_shares = HashMap::new();
        for a in self.accounts.iter() {
            for (sym, shares) in a.positions.iter() {
                let current = tot_shares.entry(sym.clone()).or_insert(0.0);
                *current += shares;
            }
        }
        tot_shares
    }

    fn can_sell_taxed(&self) -> bool {
        match self.no_taxed_sales  {
            Some(no_sales) => !no_sales,
            None => true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Account {
    name: String,
    tax_sheltered: bool,
    cash: f32,
    positions: HashMap<String, f32>
}

impl Account {
    pub fn new(name: &str) -> Account {
        let name = name.to_owned();
        Account {
            name, tax_sheltered: false, cash: 0.0, positions: HashMap::new()
        }
    }

    fn value(&self, market: &Vec<Investment>) -> f32 {
        self.cash + self.positions.iter()
            .map(|(sym, pos)| {
                match market.iter().find(|i| &i.symbol == sym) {
                    Some(info) => pos * info.price,
                    None => 0.0
                }
            })
            .sum::<f32>()
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Investment {
    symbol: String,
    price: f32,
    div_yield: Option<f32>
}

impl Investment {
    fn new(symbol: &str, price: f32) -> Investment {
        Investment {
            symbol: symbol.to_owned(), price, div_yield: None
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Results {
    positions: HashMap<String, HashMap<String, f32>>,
    allocations: HashMap<String, f32>,
    cash: HashMap<String, f32>,
    total_cash: f32,
}

impl Results {
    fn new() -> Self {
        Results {
            total_cash: 0.0, positions: HashMap::new(), allocations: HashMap::new(), cash: HashMap::new()
        }
    }

    pub fn from_positions(accounts: &Vec<Account>) -> Results {
        let mut r = Results::new();
        for a in accounts {
            r.positions.insert(a.name.clone(), a.positions.clone());
            r.cash.insert(a.name.clone(), a.cash);
        }
        r
    }

    pub fn buy_maybe(&mut self, account: &str, symbol: &str, price: f32, shares: f32) -> Option<()> {
        let cash = self.cash(account, 0.0);
        let gross = price * shares;
        if gross > cash {
            return None;
        }
        self.cash(account, -1.0 * gross);
        self.transact(account, symbol, shares);
        Some(())
    }

    fn transact(&mut self, account: &str, symbol: &str, shares: f32) -> f32 {
        let account = self.positions.entry(account.to_string()).or_insert(HashMap::new());
        let current = account.entry(symbol.to_string()).or_insert(0.0);
        *current += shares;
        if *current < 0.0 {
            *current = 0.0;
        }
        *current
    }

    fn cash(&mut self, account: &str, change: f32) -> f32 {
        let current = self.cash.entry(account.to_string()).or_insert(0.0);
        *current += change;
        *current
    }

    fn calculate_percentages(&mut self, prices: &HashMap<&String, f32>) {
        self.total_cash = self.cash.iter().map(|(_, c)| c).sum();
        let mut total = self.total_cash;

        for (_, positions) in self.positions.iter() {
            for (sym, shares) in positions.iter() {
                let price = *prices.get(sym).expect("unexpected missing price");
                let gross = price * shares;
                total += gross;
                self.allocations.entry(sym.to_string())
                    .and_modify(|g| *g += gross)
                    .or_insert(gross);
            }
        }

        if total > 0.0 {
            for (_, gross) in self.allocations.iter_mut() {
                *gross = *gross / total;
            }
            self.allocations.insert(String::from("cash"), self.total_cash / total);
        }
    }
}

#[cfg(test)]
mod test {
    use spectral::prelude::*;
    use super::*;

    #[test]
    fn test_portfolio_validation_alloc() {
        let mut portfolio = Portfolio::new();
        portfolio.market.push(Investment::new("A", 1.0));
        assert_that(&portfolio.validate())
            .is_some()
            .is_equal_to("Allocations must add up to 1.0");

        portfolio.target.insert("A".to_string(), 1.001);
        assert_that(&portfolio.validate()).is_none();
    }

    #[test]
    fn portfolio_can_sell_taxed() {
        let mut p = Portfolio::new();
        assert!(p.can_sell_taxed());

        p.no_taxed_sales = Some(false);
        assert!(p.can_sell_taxed());

        p.no_taxed_sales = Some(true);
        assert!(!p.can_sell_taxed());
    }

    #[test]
    fn test_portfolio_validation_market() {
        let mut portfolio = Portfolio::new();
        portfolio.target.insert("B".to_string(), 1.001);
        let mut a = Account::new("a");
        a.positions.insert("A".to_string(), 5.0);
        portfolio.accounts.push(a);

        assert_that(&portfolio.validate())
            .is_some()
            .is_equal_to("Missing prices for some investments");

        portfolio.market.push(Investment::new("A", 1.0));
        portfolio.market.push(Investment::new("B", 1.0));
        assert_that(&portfolio.validate()).is_none();
    }

    #[test]
    fn test_portfolio_shares() {
        let mut portfolio = Portfolio::new();
        let mut a = Account::new("a");
        a.positions.insert("A".to_string(), 5.0);
        a.positions.insert("B".to_string(), 10.0);
        let mut b = Account::new("b");
        b.positions.insert("B".to_string(), 20.0);
        portfolio.accounts.push(a);
        portfolio.accounts.push(b);

        let shares = portfolio.total_shares();
        assert_that(shares.get("A").unwrap()).is_close_to(5.0, 0.001);
        assert_that(shares.get("B").unwrap()).is_close_to(30.0, 0.001);
    }

    #[test]
    fn test_account_value() {
        let market = vec!();
        let mut account = Account::new("a");
        assert_eq!(account.value(&market), 0.0);

        account.cash = 1.0;
        assert_eq!(account.value(&market), account.cash);

        let market = vec!(Investment::new("VEU", 10.0), Investment::new("BD", 100.0));
        account.positions.insert("VEU".to_string(), 3.0);
        account.positions.insert("BD".to_string(), 1.0);
        account.positions.insert("NO-PRICE".to_string(), 5.0);
        assert_eq!(account.value(&market), 131.0);
    }

    #[test]
    fn test_result_allocations() {
        let a = String::from("A");
        let b = String::from("B");
        let mut market = HashMap::new();
        market.insert(&a, 10.0);
        market.insert(&b, 1.0);

        let mut r = Results::new();
        r.transact("a1", "A", 1.0);

        r.calculate_percentages(&market); // TODO: fix type of market
        check_allocation(&r, "A", 1.0);

        r.cash.insert(String::from("a1"), 50.0);
        r.transact("a2", "A", 3.0);
        r.transact("a1", "B", 10.0);
        r.calculate_percentages(&market);

        check_allocation(&r, "A", 0.4);
        check_allocation(&r, "B", 0.1);
        check_allocation(&r, "cash", 0.5);
    }

    pub fn check_allocation(r: &Results, sym: &str, expected: f32) {
        let a = r.allocations.get(sym).expect("missing symbol");
        assert_that(a).is_close_to(expected, 0.01);
    }
}
