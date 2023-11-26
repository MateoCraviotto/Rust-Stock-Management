use std::str::FromStr;

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
struct PurchaseNumber(u32);

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub enum PurchaseState {
    Confirm(u128),
    Commit(u128),
    Cancel(u128),
}

impl ToString for PurchaseState {
    fn to_string(&self) -> String {
        match self {
            PurchaseState::Confirm(id) => format!("Confirm {}", id),
            PurchaseState::Commit(id) => format!("Commit {}", id),
            PurchaseState::Cancel(id) => format!("Cancel {}", id),
        }
    }
}

impl FromStr for PurchaseState {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().split_whitespace().collect::<Vec<&str>>()[..] {
            ["Confirm", id] => Ok(PurchaseState::Confirm(u128::from_str(id)?)),
            ["Commit", id] => Ok(PurchaseState::Commit(u128::from_str(id)?)),
            ["Cancel", id] => Ok(PurchaseState::Cancel(u128::from_str(id)?)),
            _ => Err(anyhow::anyhow!(
                "The given string is not a valid PurchaseState"
            )),
        }
    }
}