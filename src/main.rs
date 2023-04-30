#![warn(missing_docs)]

//! A transmit-only, singlethreaded, single-port with no server-side dynamic ports, TFTP server.

mod config;
mod convert;
mod message;
mod packet;
mod server;
mod state;

pub use config::Config;
pub use convert::Convert;
pub use message::Message;
pub use packet::ErrorCode;
pub use packet::Opcode;
pub use packet::OptionType;
pub use packet::Packet;
pub use packet::TransferOption;
pub use server::Server;
pub use state::State;

use std::{env, process};

fn main() {
    let config = Config::new(env::args()).unwrap_or_else(|err| {
        eprintln!("Problem parsing arguments: {err}");
        process::exit(1)
    });

    let mut server = Server::new(&config).unwrap_or_else(|err| {
        eprintln!(
            "Problem creating server on {}:{}: {err}",
            config.ip_address, config.port
        );
        process::exit(1)
    });

    println!(
        "Running TFTP Server on {}:{} in {}",
        config.ip_address,
        config.port,
        config.directory.display()
    );

    server.listen();
}
