#[derive(Deserialize)]
pub struct Accounts {
}

#[derive(Serialize)]
pub struct Results {

}

pub fn run_balancing(accounts: Accounts) -> Results {
    Results {}
}


#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
