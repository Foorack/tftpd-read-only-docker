use crate::state::{parse_options, StateOptions, Window};
use crate::{Config, Message, State};
use crate::{ErrorCode, Packet, TransferOption};
use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::Read;
use std::net::{SocketAddr, UdpSocket};
use std::path::{Path, PathBuf};

/// Server `struct` is used for handling incoming TFTP requests.
///
/// This `struct` is meant to be created by [`Server::new()`]. See its
/// documentation for more.
///
/// # Example
///
/// ```rust
/// // Create the TFTP server.
/// use tftpd::{Config, Server};
///
/// let args = ["/", "-p", "1234"].iter().map(|s| s.to_string());
/// let config = Config::new(args).unwrap();
/// let server = Server::new(&config).unwrap();
/// ```
pub struct Server {
    socket: UdpSocket,
    directory: PathBuf,
    connmap: HashMap<SocketAddr, State>,
}

impl Server {
    /// Creates the TFTP Server with the supplied [`Config`].
    pub fn new(config: &Config) -> Result<Server, Box<dyn Error>> {
        let socket = UdpSocket::bind(SocketAddr::from((config.ip_address, config.port)))?;

        let server = Server {
            socket,
            directory: config.directory.clone(),
            connmap: HashMap::new(),
        };

        Ok(server)
    }

    /// Starts listening for connections. Note that this function does not finish running until termination.
    pub fn listen(&mut self) {
        loop {
            if let Ok((packet, from)) = Message::recv_from(&self.socket) {
                match packet {
                    Packet::Rrq {
                        filename,
                        mode: _,
                        options,
                    } => {
                        if let Err(err) = self.handle_rrq(filename, options, &from) {
                            eprintln!("{from}: Error while sending file: {err}")
                        }
                    }
                    Packet::Ack(block) => {
                        if let Err(err) = self.handle_ack(block, &from) {
                            eprintln!("{from}: Error while handling ack: {err}")
                        }
                    }
                    Packet::Error { code, msg } => {
                        println!("{from}: Received ERROR {code}: {msg}");
                    }
                    _ => {
                        eprintln!("{from}: Received invalid packet {packet}");
                        if let Err(err) = Message::send_error(
                            &self.socket,
                            &from,
                            ErrorCode::IllegalOperation,
                            "invalid request",
                        ) {
                            eprintln!("{from}: Error while sending error: {err}")
                        }
                    }
                };
            }
        }
    }

    fn handle_rrq(
        &mut self,
        filename: String,
        mut options: Vec<TransferOption>,
        to: &SocketAddr,
    ) -> Result<(), Box<dyn Error>> {
        let file_path = &self.directory.join(filename);
        match check_file_exists(file_path, &self.directory) {
            ErrorCode::FileNotFound => {
                return Message::send_error(
                    &self.socket,
                    to,
                    ErrorCode::FileNotFound,
                    "file does not exist",
                );
            }
            ErrorCode::AccessViolation => {
                return Message::send_error(
                    &self.socket,
                    to,
                    ErrorCode::AccessViolation,
                    "file access violation",
                );
            }
            ErrorCode::FileExists => {
                () // OK for sending
            }
            _ => {
                return Message::send_error(
                    &self.socket,
                    to,
                    ErrorCode::NotDefined,
                    "unexpected error",
                );
            }
        }

        let state_options = parse_options(&mut options, file_path.metadata()?.len() as usize)?;
        let file = File::open(&file_path)?;
        let state = State {
            file,
            filepath: file_path.to_path_buf(),
            options: state_options,
            block_number: if options.len() > 0 { 0 } else { 1 },
            window: Window::new(),
            finished: false,
        };

        self.connmap.insert(*to, state);

        if options.len() > 0 {
            // Send OACK
            if let Err(err) = Message::send_oack(&self.socket, to, options) {
                eprintln!("{to}: Error while sending OACK: {err}");
            }
        } else {
            // Send first Data
            if let Err(err) = self.process_send(to) {
                eprintln!("{to}: Error while sending first data: {err}");
            }
        }

        return Ok(());
    }

    fn fill_window(
        window: &mut Window,
        options: &StateOptions,
        mut file: &File,
    ) -> Result<bool, Box<dyn Error>> {
        let current = window.len() as u16;
        let windowsize = options.windowsize;
        let blk_size = options.blk_size;

        // If e.g. window has 3 chunks and windowsize is 4, we need to fill 1 more chunk
        // Return false if
        let to_fill = windowsize - current;
        if to_fill == 0 {
            return Ok(false);
        }

        let mut unfilled = false;
        for _ in 0..to_fill {
            let mut buf = vec![0; blk_size];
            let read = file.read(&mut buf)?;
            if read == 0 || read < blk_size {
                unfilled = true;
            }
            if read == 0 {
                break;
            }
            if read < blk_size {
                buf.truncate(read);
            }
            window.push(buf);
        }

        return Ok(unfilled);
    }

    fn handle_ack(&mut self, ack_block_number: u16, to: &SocketAddr) -> Result<(), Box<dyn Error>> {
        let state = self.connmap.get_mut(to).ok_or("missing state")?;
        let windowsize = state.options.windowsize;
        let diff = ack_block_number.wrapping_sub(state.block_number);
        println!("{to}: Received ack {ack_block_number} (diff {diff}) (ws={windowsize})");
        if diff <= windowsize {
            state.block_number = ack_block_number.wrapping_add(1);
            // If diff is 3, then pop 3 elements from state.window
            for _ in 0..(diff + 1) {
                state.window.pop();
            }
        }

        if state.finished {
            return self.end_session(to);
        }

        return self.process_send(to);
    }

    fn end_session(&mut self, to: &SocketAddr) -> Result<(), Box<dyn Error>> {
        let state = self.connmap.get(to).ok_or("missing state")?;
        let filepath: &String = &state.filepath.display().to_string();
        println!("{to}: Sent file {filepath}");
        self.connmap.remove(to);
        return Ok(());
    }

    fn process_send(&mut self, to: &SocketAddr) -> Result<(), Box<dyn Error>> {
        let state = self.connmap.get_mut(to).unwrap();
        state.finished = Self::fill_window(&mut state.window, &state.options, &state.file)?;
        Self::send_window(&self.socket, to, &state.window, state.block_number)
    }

    fn send_window(
        socket: &UdpSocket,
        to: &SocketAddr,
        window: &Window,
        mut block_num: u16,
    ) -> Result<(), Box<dyn Error>> {
        for frame in window {
            let size = frame.len();
            println!("{to}: Sending block {block_num} with {size} bytes");
            Message::send_data(socket, to, block_num, frame.to_vec())?;
            block_num = block_num.wrapping_add(1);
        }

        Ok(())
    }
}

fn check_file_exists(file: &Path, directory: &PathBuf) -> ErrorCode {
    if !validate_file_path(file, directory) {
        return ErrorCode::AccessViolation;
    }

    if !file.exists() {
        return ErrorCode::FileNotFound;
    }

    ErrorCode::FileExists
}

fn validate_file_path(file: &Path, directory: &PathBuf) -> bool {
    !file.to_str().unwrap().contains("..") && file.ancestors().any(|a| a == directory)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_file_path() {
        assert!(validate_file_path(
            &PathBuf::from("/dir/test/file"),
            &PathBuf::from("/dir/test")
        ));

        assert!(!validate_file_path(
            &PathBuf::from("/system/data.txt"),
            &PathBuf::from("/dir/test")
        ));

        assert!(!validate_file_path(
            &PathBuf::from("~/some_data.txt"),
            &PathBuf::from("/dir/test")
        ));

        assert!(!validate_file_path(
            &PathBuf::from("/dir/test/../file"),
            &PathBuf::from("/dir/test")
        ));
    }
}
