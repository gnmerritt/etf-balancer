use super::*;

pub fn run_balancing(_accounts: Portfolio) -> Results {
    Results::new()
}

#[cfg(test)]
mod single_account {
    use super::*;

    #[test]
    fn check_empty_case() {
        assert_eq!(run_balancing(Portfolio::new()), Results::new());
    }
}

#[cfg(test)]
mod multiple_accounts {
    // TODO
}
