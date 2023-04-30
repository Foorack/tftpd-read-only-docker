use std::{
    error::Error,
    fs::File,
    net::{SocketAddr, UdpSocket},
    path::PathBuf,
    thread,
    time::{Duration, SystemTime},
};

use crate::{ErrorCode, Message, OptionType, Packet, TransferOption, Window};

pub struct WorkState {
    pub file: PathBuf,
    pub options: Vec<TransferOption>,
}

pub struct Worker;

#[derive(PartialEq, Eq)]
enum WorkType {
    Send(u64),
}

const MAX_RETRIES: u32 = 6;
const DEFAULT_TIMEOUT_SECS: u64 = 5;
const TIMEOUT_BUFFER_SECS: u64 = 1;
const DEFAULT_BLOCK_SIZE: usize = 512;

impl Worker {
    /// Sends a file to the remote [`SocketAddr`] that has sent a read request using
    /// a random port, asynchronously.
    pub fn send(
        addr: SocketAddr,
        remote: SocketAddr,
        file_path: PathBuf,
        mut options: Vec<TransferOption>,
    ) {
        thread::spawn(move || {
            let mut handle_send = || -> Result<(), Box<dyn Error>> {
                let work_type = WorkType::Send(file_path.metadata()?.len());
                let worker_options = parse_options(&mut options, &work_type)?;

                accept_request(&socket, &options, &work_type)?;
                send_file(&socket, File::open(&file_path)?, &worker_options)?;

                Ok(())
            };

            match handle_send() {
                Ok(_) => {
                    println!(
                        "Sent {} to {}",
                        file_path.file_name().unwrap().to_str().unwrap(),
                        remote
                    );
                }
                Err(err) => {
                    eprintln!("{err}");
                }
            }
        });
    }
}

fn send_file(
    socket: &UdpSocket,
    file: File,
    worker_options: &WorkerOptions,
) -> Result<(), Box<dyn Error>> {
    let mut block_number = 1;
    let mut window = Window::new(worker_options.windowsize, worker_options.blk_size, file);

    loop {
        let filled = window.fill()?;

        let mut retry_cnt = 0;
        let mut time =
            SystemTime::now() - Duration::from_secs(DEFAULT_TIMEOUT_SECS + TIMEOUT_BUFFER_SECS);
        loop {
            if time.elapsed()? >= Duration::from_secs(DEFAULT_TIMEOUT_SECS) {
                send_window(socket, &window, block_number)?;
                time = SystemTime::now();
            }

            match Message::recv(socket) {
                Ok(Packet::Ack(received_block_number)) => {
                    let diff = received_block_number.wrapping_sub(block_number);
                    if diff <= worker_options.windowsize {
                        block_number = received_block_number.wrapping_add(1);
                        window.remove(diff + 1)?;
                        break;
                    }
                }
                Ok(Packet::Error { code, msg }) => {
                    return Err(format!("Received error code {code}: {msg}").into());
                }
                _ => {
                    retry_cnt += 1;
                    if retry_cnt == MAX_RETRIES {
                        return Err(format!("Transfer timed out after {MAX_RETRIES} tries").into());
                    }
                }
            }
        }

        if !filled && window.is_empty() {
            break;
        }
    }

    Ok(())
}
