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
    fn new() -> Self {
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

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Account {
    name: String,
    tax_sheltered: bool,
    cash: f32,
    positions: HashMap<String, f32>
}

impl Account {
    fn new(name: &str) -> Account {
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
    cash: f32
}

impl Results {
    fn new() -> Self {
        Results {
            cash: 0.0, positions: HashMap::new()
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
}
