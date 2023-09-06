mod descriptor;
mod error;
mod key;
mod network;
mod parse;
mod rpc;
mod spend;
mod state;

use std::path::PathBuf;

use elements_miniscript as miniscript;
use miniscript::bitcoin;
use simplicity::human_encoding;

use crate::error::Error;
use crate::key::DescriptorSecretKey;
use crate::network::Network;
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
    ImportProgram { path: PathBuf },
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
            let address = state.next_address()?;
            println!("{}", address);
            state.save("state.json", false)?;
        }
        Command::GetBalance => {
            let state = State::load("state.json")?;
            let balance = spend::get_balance(&state)?;
            println!("{}", balance);
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
        Command::ImportProgram { path } => {
            let file = std::fs::read_to_string(path)?;
            let forest = human_encoding::Forest::<simplicity::jet::Elements>::parse(&file)?;
            let cmr = forest.roots()["main"].cmr();

            let mut state = State::load("state.json")?;
            println!("New CMR: {}", cmr);
            state.add_cmr(cmr);
            state.save("state.json", false)?;
        }
    }

    Ok(())
}
