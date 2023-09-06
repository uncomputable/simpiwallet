use std::str::FromStr;

use lexopt::prelude::*;

use crate::error::Error;
use crate::rpc::Connection;
use crate::spend::Payment;
use crate::Command;

const HELP: &str = "Usage: simpiwallet [new | getnewaddress | getbalance | sendtoaddress | setfee | setrpc | setnetwork | importprogram | help]";
const NEW_HELP: &str = "simpiwallet new";
const GET_NEW_ADDRESS_HELP: &str = "simpiwallet getnewaddress";
const GET_BALANCE_HELP: &str = "simpiwallet getbalance";
const SEND_TO_ADDRESS_HELP: &str = "simpiwallet sendtoaddress ADDRESS AMOUNT";
const SET_FEE_HELP: &str = "simpiwallet setfee AMOUNT";
const SET_RPC_HELP: &str = "simpiwallet setrpc URL PORT USERNAME [PASSWORD]";
const SET_NETWORK_HELP: &str = "simpiwallet setnetwork [regtest | testnet]";
const IMPORT_PROGRAM_HELP: &str = "simpiwallet importprogram PATH";
const HELP_HELP: &str =
    "simpiwallet help [new | getnewaddress | getbalance | sendtoaddress | setfee | setrpc | setnetwork | importprogram]";

pub fn command() -> Result<Command, Error> {
    let mut parser = lexopt::Parser::from_env();
    let arg = parser.next()?.ok_or(Error::missing_value("subcommand"))?;

    match arg {
        Value(command) => {
            let command = command.string()?;
            match command.as_str() {
                "new" => Ok(Command::New),
                "getnewaddress" => Ok(Command::GetNewAddress),
                "getbalance" => Ok(Command::GetBalance),
                "sendtoaddress" => {
                    let address = argument(&mut parser, "address")?;
                    let amount = argument(&mut parser, "amount")?;
                    let send_to = Payment { address, amount };
                    Ok(Command::SendToAddress { send_to })
                }
                "setfee" => {
                    let fee = argument(&mut parser, "amount")?;
                    Ok(Command::SetFee { fee })
                }
                "setrpc" => {
                    let url = argument(&mut parser, "url")?;
                    let user = argument(&mut parser, "user")?;
                    let pass = optional_argument(&mut parser)?;
                    let rpc = Connection { url, user, pass };
                    Ok(Command::SetRpc { rpc })
                }
                "setnetwork" => {
                    let network = argument(&mut parser, "network")?;
                    Ok(Command::SetNetwork { network })
                }
                "importprogram" => {
                    let path = argument(&mut parser, "path")?;
                    Ok(Command::ImportProgram { path })
                }
                "help" => {
                    let help = match optional_argument::<String>(&mut parser)?.as_deref() {
                        Some("new") => NEW_HELP,
                        Some("getnewaddress") => GET_NEW_ADDRESS_HELP,
                        Some("getbalance") => GET_BALANCE_HELP,
                        Some("sendtoaddress") => SEND_TO_ADDRESS_HELP,
                        Some("setfee") => SET_FEE_HELP,
                        Some("setrpc") => SET_RPC_HELP,
                        Some("setnetwork") => SET_NETWORK_HELP,
                        Some("importprogram") => IMPORT_PROGRAM_HELP,
                        Some("help") => HELP_HELP,
                        _ => HELP,
                    };

                    println!("{}", help);
                    std::process::exit(0);
                }
                command => Err(Error::unknown_command(command)),
            }
        }
        Long("help") => {
            println!("{}", HELP);
            std::process::exit(0);
        }
        _ => Err(arg.unexpected().into()),
    }
}

fn argument<A>(parser: &mut lexopt::Parser, name: &str) -> Result<A, Error>
where
    A: FromStr,
    <A as FromStr>::Err: ToString,
{
    let arg = parser.next()?.ok_or(Error::missing_value(name))?;

    if let Value(os_str) = arg {
        let str = os_str.string()?;
        let a = A::from_str(&str).map_err(|e| Error::CouldNotParse(e.to_string()))?;
        Ok(a)
    } else {
        Err(arg.unexpected().into())
    }
}

fn optional_argument<A>(parser: &mut lexopt::Parser) -> Result<Option<A>, Error>
where
    A: FromStr,
    <A as FromStr>::Err: ToString,
{
    let arg = match parser.next()? {
        Some(arg) => arg,
        None => return Ok(None),
    };

    if let Value(os_str) = arg {
        let str = os_str.string()?;
        let a = A::from_str(&str).map_err(|e| Error::CouldNotParse(e.to_string()))?;
        Ok(Some(a))
    } else {
        Err(arg.unexpected().into())
    }
}
