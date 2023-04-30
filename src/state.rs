use std::{error::Error, fs::File, path::PathBuf};

use crate::{OptionType, TransferOption};

pub type Chunk = Vec<u8>;
pub type Window = Vec<Chunk>;

/// Window `struct` is used to store chunks of data from a file.
/// It is used to store the data that is being sent for Windowsize option.
pub struct State {
    pub(crate) file: File,
    pub(crate) filepath: PathBuf,
    pub(crate) options: StateOptions,
    pub(crate) block_number: u16,
    pub(crate) window: Window,
    pub(crate) finished: bool,
}

// const MAX_RETRIES: u32 = 6;
const DEFAULT_TIMEOUT_SECS: u64 = 5;
// const TIMEOUT_BUFFER_SECS: u64 = 1;
const DEFAULT_BLOCK_SIZE: usize = 512;

#[derive(Debug, PartialEq, Eq)]
pub struct StateOptions {
    pub blk_size: usize,
    pub t_size: usize,
    pub timeout: u64,
    pub windowsize: u16,
}

pub fn parse_options(
    options: &mut Vec<TransferOption>,
    file_size: usize,
) -> Result<StateOptions, Box<dyn Error>> {
    let mut state_options = StateOptions {
        blk_size: DEFAULT_BLOCK_SIZE,
        t_size: 0,
        timeout: DEFAULT_TIMEOUT_SECS,
        windowsize: 1,
    };

    for option in &mut *options {
        let TransferOption { option, value } = option;

        match option {
            OptionType::BlockSize => state_options.blk_size = *value,
            OptionType::TransferSize => {
                *value = file_size;
                state_options.t_size = file_size;
            }
            OptionType::Timeout => {
                if *value == 0 {
                    return Err("Invalid timeout value".into());
                }
                state_options.timeout = *value as u64;
            }
            OptionType::Windowsize => {
                if *value == 0 || *value > u16::MAX as usize {
                    return Err("Invalid windowsize value".into());
                }
                state_options.windowsize = *value as u16;
            }
        }
    }

    Ok(state_options)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_send_options() {
        let mut options = vec![
            TransferOption {
                option: OptionType::BlockSize,
                value: 1024,
            },
            TransferOption {
                option: OptionType::TransferSize,
                value: 0,
            },
            TransferOption {
                option: OptionType::Timeout,
                value: 5,
            },
        ];

        let worker_options = parse_options(&mut options, 12345).unwrap();

        assert_eq!(options[0].value, worker_options.blk_size);
        assert_eq!(12345, worker_options.t_size);
        assert_eq!(options[2].value as u64, worker_options.timeout);
    }

    #[test]
    fn parses_default_options() {
        assert_eq!(
            parse_options(&mut vec![], 12345678).unwrap(),
            StateOptions {
                blk_size: DEFAULT_BLOCK_SIZE,
                t_size: 12345678,
                timeout: DEFAULT_TIMEOUT_SECS,
                windowsize: 1,
            }
        );
    }
}
