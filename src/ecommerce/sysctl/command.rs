use std::str::FromStr;

use anyhow::{anyhow, bail, Ok};
use crate::common::order::Order;

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Shutdown,
    NetUp,
    NetDown,
    Sell(Order),
    SellFromFile(String)
}

impl FromStr for Command{
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let clean = s.trim_start();
        let first = clean.as_bytes().get(0).ok_or(anyhow!("The command must be at least 1 ASCII character long with"))?;
        match first{
            b'S' | b's' => Ok(Command::Shutdown),
            b'U' | b'u' => Ok(Command::NetUp),
            b'D' | b'd' => Ok(Command::NetDown),
            b'O' | b'o' => Ok(Command::Sell(Order::from_str(&clean[1..])?)),
            b'F' | b'f' => Ok(Command::SellFromFile(parse_file(&clean[1..])?)),
            _ => bail!("Valid Commands are: S (Shutdown), U (NetUp), D (NetDown), O (Order) <Order>, F (Orders From File) <FilePath>")
        }
    }
}

fn parse_file(s: &str) -> anyhow::Result<String> {
    let trimmed = s.trim();
    if trimmed.is_empty() { bail!("A path is needed when reading orders from a file")}
    Ok(String::try_from(trimmed)?)
}

#[cfg(test)]
mod test{
    use std::str::FromStr;

    use crate::{common::order::Order, ecommerce::sysctl::command::Command};

    #[test]
    fn cmd_from_correct_upper() {
        let shutdown = Command::from_str("S").expect("Command was not properly parsed");
        let netup = Command::from_str("U").expect("Command was not properly parsed");
        let netdown = Command::from_str("D").expect("Command was not properly parsed");
        let order = Command::from_str("O 1,1").expect("Command was not properly parsed");
        let from_file = Command::from_str("F Something").expect("Command was not properly parsed");

        assert_eq!(Command::Shutdown, shutdown);
        assert_eq!(Command::NetUp, netup);
        assert_eq!(Command::NetDown, netdown);
        assert_eq!(Command::Sell(Order::new(1u64, 1u64)), order);
        assert_eq!(Command::SellFromFile("Something".to_owned()), from_file);
    }

    #[test]
    fn cmd_from_correct_lower() {
        let shutdown = Command::from_str("s").expect("Command was not properly parsed");
        let netup = Command::from_str("u").expect("Command was not properly parsed");
        let netdown = Command::from_str("d").expect("Command was not properly parsed");
        let order = Command::from_str("o 1,1").expect("Command was not properly parsed");
        let from_file = Command::from_str("f Something").expect("Command was not properly parsed");

        assert_eq!(Command::Shutdown, shutdown);
        assert_eq!(Command::NetUp, netup);
        assert_eq!(Command::NetDown, netdown);
        assert_eq!(Command::Sell(Order::new(1u64, 1u64)), order);
        assert_eq!(Command::SellFromFile("Something".to_owned()), from_file);
    }

    #[test]
    fn cmd_from_correct_spaces() {
        let shutdown = Command::from_str("   s").expect("Command was not properly parsed");
        let netup = Command::from_str("  u").expect("Command was not properly parsed");
        let netdown = Command::from_str("   d").expect("Command was not properly parsed");
        let order = Command::from_str("   o    1   ,   1").expect("Command was not properly parsed");
        let from_file = Command::from_str("   f     Something").expect("Command was not properly parsed");

        assert_eq!(Command::Shutdown, shutdown);
        assert_eq!(Command::NetUp, netup);
        assert_eq!(Command::NetDown, netdown);
        assert_eq!(Command::Sell(Order::new(1u64, 1u64)), order);
        assert_eq!(Command::SellFromFile("Something".to_owned()), from_file);
    }
}
