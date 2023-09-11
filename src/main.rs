mod descriptor;
mod error;
mod key;
mod network;
mod parse;
mod rpc;
mod spend;
mod state;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use elements::hex::FromHex;
use elements_miniscript as miniscript;
use miniscript::{bitcoin, elements};
use simplicity::{human_encoding, Value};

use crate::error::Error;
use crate::key::DescriptorSecretKey;
use crate::network::Network;
use crate::parse::Choice;
use crate::spend::Payment;
use crate::state::State;

pub enum Command {
    New,
    GetNewAddress,
    GetBalance,
    SendToAddress { send_to: Payment },
    SetFee { fee: bitcoin::Amount },
    SetRpc { rpc: rpc::Connection },
    SetNetwork { network: Network },
    ImportProgram { program: PathBuf },
    SatisfyProgram { program: PathBuf, witness: PathBuf },
}

fn main() -> Result<(), Error> {
    let command = parse::command()?;

    match command {
        Command::New => {
            let xpriv = DescriptorSecretKey::random()?;
            let state = State::new(xpriv);
            println!("Generating state.json");
            state.save("state.json", true)?;
        }
        Command::GetNewAddress => {
            let mut state = State::load("state.json")?;

            let mut asm: Vec<_> = state.assembly().iter().collect();
            asm.sort();

            let address = if !asm.is_empty()
                && parse::prompt::<Choice>("Address of assembly fragment? y/n: ")?.into()
            {
                for (index, cmr) in asm.iter().enumerate() {
                    println!("{}: {}", index, cmr);
                }

                let index: usize = parse::prompt("Assembly fragment index: ")?;
                let cmr = asm.get(index).ok_or(Error::AssemblyOutOfBounds)?;
                state
                    .assembly()
                    .get_address(cmr, state.network().address_params())
                    .expect("set contains cmr")
            } else {
                state.next_address()?
            };

            println!("{}", address);
            state.save("state.json", false)?;
        }
        Command::GetBalance => {
            let state = State::load("state.json")?;
            let spendable_balance = spend::get_spendable_balance(&state)?;
            let locked_balance = spend::get_locked_balance(&state)?;
            println!("Spendable: {}", spendable_balance);
            println!("Locked:    {}", locked_balance);
        }
        Command::SendToAddress { send_to } => {
            let mut state = State::load("state.json")?;
            let txid = spend::send_to_address(&mut state, send_to)?;
            println!("{}", txid);
            state.save("state.json", false)?;
        }
        Command::SetFee { fee } => {
            let mut state = State::load("state.json")?;
            state.set_fee(fee);
            println!("New fee: {}", fee);
            state.save("state.json", false)?;
        }
        Command::SetRpc { rpc } => {
            let mut state = State::load("state.json")?;
            println!("New RPC connection: {}", rpc);
            state.set_rpc(rpc);
            state.save("state.json", false)?;
        }
        Command::SetNetwork { network } => {
            let mut state = State::load("state.json")?;
            println!("New network: {}", network);
            state.set_network(network);
            state.save("state.json", false)?;
        }
        Command::ImportProgram { program } => {
            let file = std::fs::read_to_string(program)?;
            let forest = human_encoding::Forest::<simplicity::jet::Elements>::parse(&file)?;
            let cmr = forest.roots()["main"].cmr();

            let mut state = State::load("state.json")?;
            if state.assembly_mut().insert(cmr) {
                println!("New CMR: {}", cmr);
            }
            state.save("state.json", false)?;
        }
        Command::SatisfyProgram { program, witness } => {
            let mut state = State::load("state.json")?;

            let file = std::fs::read_to_string(program)?;
            let forest = human_encoding::Forest::<simplicity::jet::Elements>::parse(&file)?;
            let cmr = forest.roots()["main"].cmr();

            if !state.assembly().contains(cmr) {
                return Err(Error::UnknownAssembly(cmr))?;
            }

            let file = std::fs::read_to_string(witness)?;
            let name_to_hex: HashMap<String, String> = serde_json::from_str(&file)?;
            let name_to_value = name_to_hex
                .into_iter()
                .map(|(name, hex)| {
                    Vec::<u8>::from_hex(&hex)
                        .map_err(|err| Error::CouldNotParse(err.to_string()))
                        .map(|bytes| (Arc::<str>::from(name), Value::from_slice(&bytes)))
                })
                .collect::<Result<HashMap<Arc<str>, Arc<Value>>, Error>>()?;

            let program = forest.to_witness_node(&name_to_value)?;
            let maybe_replaced = state.assembly_mut().insert_satisfaction(&program)?;

            if let Some(replaced) = maybe_replaced {
                println!("Replaced old satisfaction {}", replaced);
            }
            println!("Inserted new satisfaction\n");
            println!("Note that the wallet cannot check if the satisfaction is valid!");
            println!("It is the responsibility of the user to provide a valid satisfaction.");
            println!("The wallet will return an error if the satisfaction fails during spending.");

            state.save("state.json", false)?;
        }
    }

    Ok(())
}
