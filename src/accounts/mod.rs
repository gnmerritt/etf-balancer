pub mod balancer;

use std::collections::HashMap;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Portfolio {
    target: HashMap<String, f32>,
    accounts: Vec<Account>,
    market: Vec<Investment>
}

impl Portfolio {
    fn new() -> Self {
        Portfolio {
            target: HashMap::new(), accounts: vec!(), market: vec!()
        }
    }

    pub fn validate(&self) -> Option<&'static str> {
        let sum: f32 = self.target.iter().map(|(_, p)| p).sum();
        if (sum - 1.0).abs() > 0.01 {
            return Some("Allocations must add up to 1.0")
        }
        None
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
    fn test_portfolio_validation() {
        let mut portfolio = Portfolio::new();
        assert_that(&portfolio.validate())
            .is_some()
            .is_equal_to("Allocations must add up to 1.0");

        portfolio.target.insert("A".to_string(), 1.001);
        assert_that(&portfolio.validate()).is_none();
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
