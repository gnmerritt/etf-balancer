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
    taxable_txns: i32,
    positions: HashMap<String, HashMap<String, f32>>,
    cash: f32
}

impl Results {
    fn new() -> Self {
        Results {
            taxable_txns: 0, cash: 0.0, positions: HashMap::new()
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

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
        assert_eq!(account.value(&market), 131.0);
    }
}
