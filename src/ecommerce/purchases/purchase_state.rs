use std::str::FromStr;

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
struct PurchaseNumber(u32);

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub enum PurchaseState {
    Reserve,
    Confirm,
    Cancel,
}

impl ToString for PurchaseState {
    fn to_string(&self) -> String {
        match self {
            PurchaseState::Reserve => "Reserve".to_string(),
            PurchaseState::Confirm => "Confirm".to_string(),
            PurchaseState::Cancel => "Cancel".to_string(),
        }
    }
}

impl FromStr for PurchaseState {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim() {
            "Reserve" => Ok(PurchaseState::Reserve),
            "Confirm" => Ok(PurchaseState::Confirm),
            "Cancel" => Ok(PurchaseState::Cancel),
            _ => Err(anyhow::anyhow!(
                "The given string is not a valid PurchaseState"
            )),
        }
    }
}
