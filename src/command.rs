use crate::protocol::RespValue;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio::io::AsyncWriteExt;

/// each variant holds the arguments needed for that command
#[derive(Debug, Clone)]
pub enum Command {
    Set { key: String, value: Vec<u8> }, // uses Vec<u8> for binary safety
    Get { key: String },
    Del { key: String },
    Quit,
    Unknown,
}

impl Command {
    // convert from the generic RESP protocol into our specific command types
    pub fn parse_from_resp_array(array: Vec<RespValue>) -> Command {
        if array.is_empty() {
            return Command::Unknown;
        }

        // the first element of a command array is usually the command name (BulkString or SimpleString)
        let command_name = if let Some(RespValue::BulkString(cmd_bytes)) = array.get(0) {
            String::from_utf8_lossy(cmd_bytes).to_ascii_uppercase()
        } else if let Some(RespValue::SimpleString(cmd_str)) = array.get(0) {
            cmd_str.to_ascii_uppercase()
        } else {
            return Command::Unknown;
        };

        match command_name.as_str() {
            "SET" => {
                if array.len() >= 3 {
                    if let (Some(RespValue::BulkString(key_bytes)), Some(RespValue::BulkString(value_bytes))) = (array.get(1), array.get(2)) {
                        let key = String::from_utf8_lossy(key_bytes).to_string();
                        // Using RESP the value is typically a single bulk string, this means we can just take the second argument as the value.
                        // if we wanted multi word values, the client would send them as a single bluk string
                        Command::Set {
                            key,
                            value: value_bytes.clone(),
                        }
                    } else {
                        Command::Unknown // malformed SET command arguments
                    }
                } else {
                    Command::Unknown // not enough arguments
                }
            }
            "GET" => {
                if array.len() == 2 {
                    if let Some(RespValue::BulkString(key_bytes)) = array.get(1) {
                        let key = String::from_utf8_lossy(key_bytes).to_string();
                        Command::Get { key }
                    } else {
                        Command::Unknown // malformed GET command arguments
                    }
                } else {
                    Command::Unknown // wrong number of arguments
                }
            }
            "DEL" => {
                if array.len() == 2 {
                    if let Some(RespValue::BulkString(key_bytes)) = array.get(1) {
                        let key = String::from_utf8_lossy(key_bytes).to_string();
                        Command::Del { key }
                    } else {
                        Command::Unknown // malformed DEL command arguments
                    }
                } else {
                    Command::Unknown // wrong number of arguments
                }
            }
            "QUIT" => Command::Quit,
            _ => Command::Unknown,
        }
    }

    // executes the command and returns a "RespValue" as the response
    pub async fn execute(
        self,
        db: Arc<Mutex<HashMap<String, Vec<u8>>>>, 
        _socket: &mut (impl AsyncWriteExt + Unpin), // _socket is currently unused but might be useful for Pub/Sub
    ) -> Result<RespValue, Box<dyn std::error::Error + Send + Sync >> { // added Send + Sync for error handling across threads
        match self {
            Command::Set { key, value } => {
                // acquire a lock on the Mutex
                let mut db_locked = db
                    .lock()
                    .expect("Failed to acquire DB lock in SET command; Mutex might be poisoned");
                db_locked.insert(key, value);
                Ok(RespValue::SimpleString("OK".to_string()))
            }
            Command::Get { key } => {
                // acquire a lock
                let db_locked = db
                    .lock()
                    .expect("Failed to acquire DB lock in GET command; Mutex might be poisoned");
                match db_locked.get(&key) {
                    Some(value) => Ok(RespValue::BulkString(value.clone())), // return as BulkString
                    None => Ok(RespValue::Null), // Uuse Null for non-existent keys
                }
            }
            Command::Del { key } => {
                // acquire a lock
                let mut db_locked = db
                    .lock()
                    .expect("Failed to acquire DB lock in DEL command; Mutex might be poisoned");
                match db_locked.remove(&key) {
                    Some(_) => Ok(RespValue::Integer(1)), // redis DEL returns number of keys deleted
                    None => Ok(RespValue::Integer(0)), // 0 keys deleted
                }
            }
            Command::Quit => {
                Ok(RespValue::SimpleString("Connection closing shortly".to_string()))
            }
            Command::Unknown => Ok(RespValue::Error(
                "ERR unknown command or malformed command arguments".to_string(),
            )),
        }
    }
}