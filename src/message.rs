use std::{
    error::Error,
    net::{SocketAddr, UdpSocket},
};

use crate::{ErrorCode, Packet, TransferOption};

/// Message `struct` is used for easy message transmission of common TFTP
/// message types.
///
/// # Example
///
/// ```rust
/// use std::{net::{SocketAddr, UdpSocket}, str::FromStr};
/// use tftpd::{Message, ErrorCode};
///
/// // Send a FileNotFound error.
/// Message::send_error_to(
///     &UdpSocket::bind(SocketAddr::from_str("127.0.0.1:6969").unwrap()).unwrap(),
///     &SocketAddr::from_str("127.0.0.1:1234").unwrap(),
///     ErrorCode::FileNotFound,
///     "file does not exist",
/// );
/// ```
pub struct Message;

const MAX_REQUEST_PACKET_SIZE: usize = 512;

impl Message {
    /// Sends a data packet to the supplied [`SocketAddr`].
    pub fn send_data(
        socket: &UdpSocket,
        to: &SocketAddr,
        block_num: u16,
        data: Vec<u8>,
    ) -> Result<(), Box<dyn Error>> {
        socket.send_to(&Packet::Data { block_num, data }.serialize()?, to)?;

        Ok(())
    }

    /// Sends an acknowledgement packet to the supplied [`SocketAddr`].
    pub fn send_ack(
        socket: &UdpSocket,
        to: &SocketAddr,
        block_number: u16,
    ) -> Result<(), Box<dyn Error>> {
        socket.send_to(&Packet::Ack(block_number).serialize()?, to)?;

        Ok(())
    }

    /// Sends an error packet to the supplied [`SocketAddr`].
    pub fn send_error(
        socket: &UdpSocket,
        to: &SocketAddr,
        code: ErrorCode,
        msg: &str,
    ) -> Result<(), Box<dyn Error>> {
        let buf = Packet::Error {
            code,
            msg: msg.to_string(),
        };
        socket.send_to(&buf.serialize()?, to)?;

        Ok(())
    }

    /// Sends an option acknowledgement packet to the supplied [`SocketAddr`].
    pub fn send_oack(
        socket: &UdpSocket,
        to: &SocketAddr,
        options: Vec<TransferOption>,
    ) -> Result<(), Box<dyn Error>> {
        socket.send_to(&Packet::Oack(options).serialize()?, to)?;

        Ok(())
    }

    /// Receives a packet from any incoming remote request, and returns the
    /// parsed [`Packet`] and the requesting [`SocketAddr`]. This function cannot handle
    /// large data packets due to the limited buffer size, so it is intended for
    /// only accepting incoming requests.
    pub fn recv_from(socket: &UdpSocket) -> Result<(Packet, SocketAddr), Box<dyn Error>> {
        let mut buf = [0; MAX_REQUEST_PACKET_SIZE];
        let (number_of_bytes, from) = socket.recv_from(&mut buf)?;
        let packet = Packet::deserialize(&buf[..number_of_bytes])?;

        println!("{}: [Packet] {}", from, packet);

        Ok((packet, from))
    }
}
