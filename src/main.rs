use std::{
    collections::HashMap, // in memory key-value store
    sync::{Arc, Mutex},   // Arc for shared ownership, Mutex for mutual exclusion
};
use tokio::{
    io::AsyncWriteExt, // asynchronous write operations // removed read
    net::{TcpListener, TcpStream},     // TcpListener to accept incoming connections, TcpStream for individual connections
};
// import our modules

mod protocol; // RespValue enum and RESP serialization/deserialization logic
mod command;  //  Command enum and command parsing/execution logic

use protocol::RespValue;
use command::Command;

#[tokio::main] // mark this as the main function for a tokio runtime
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // create in memory HashMap to store key-value pairs with arc<mutex for safe, shared and mutable access to it 
    // changed to Vec<u8> to be binary safe
    let db: Arc<Mutex<HashMap<String, Vec<u8>>>> = Arc::new(Mutex::new(HashMap::new()));
    // bind the TcpListener to local address with default redis port
    let listener = TcpListener::bind("127.0.0.1:6379").await?;
    println!("Server listening on 127.0.0.1:6379");

    // loop to accept client connections
    loop {
        // wait for a new client connection.
        // `accept()` returns a `TcpStream` and a `SocketAddr` 
        let (mut socket, addr) = listener.accept().await?;
        println!("Accepted connection from: {}", addr);

        // clone Arc to give each new task its own reference to the shared HashMap
        let db_clone = Arc::clone(&db);

        // start a new asynchronous task for each incoming client conn
        tokio::spawn(async move {
            // call the client handler function.
            if let Err(e) = handle_client(&mut socket, db_clone).await {
                eprintln!("Error handling client {}: {}", addr, e);
            }
            println!("Client disconnected: {}", addr);
        });
    }
}

/// handles a single client conn, reads commands from client, processes them and responds
async fn handle_client(
    socket: &mut TcpStream,
    db: Arc<Mutex<HashMap<String, Vec<u8>>>>, //changed to Vec<u8>
) -> Result<(), Box<dyn std::error::Error>> {
        loop {
        // attempt to parse a RESP value from a tcp stream
        let resp_value = RespValue::from_stream(socket).await;

        let cmd: Command;
        let response: RespValue;

        match resp_value{
            Ok(Some(RespValue::Array(array))) => {
                // successfuly parsed array, now parse it into our internal command enum
                cmd = Command::parse_from_resp_array(array);
                println!("Parsed command: {:?}", cmd); // log parsed command

                // execute command and get RESP formatted response
                let exec_result = cmd.clone().execute(Arc::clone(&db), socket).await;
                match exec_result {
                    Ok(res) => response = res,
                    Err(e) => {
                        // Propagate the error directly
                        return Err(e);
                    }
                }
            }
            Ok(Some(_other_value)) => {
                // if it's not an array (eg: simple string), it's a malformed command
                response = RespValue::Error("ERR Protocol error: expected array".to_string());
                cmd = Command::Unknown; // mark as unknwon command for logging purposes
            }
            Ok(None) => {
                // if the stream is closed, break out of loop and close connection
                return Ok(());
            }
            Err(e) => {
                // handle error parsing RESP value
                eprintln!("RESP parse error: {}", e);
                response = RespValue::Error(format!("ERR Protocol parsing error: {}", e));
                cmd = Command::Unknown; // mark as unknown command for logging purposes
            }
        }

        // send the response back to the client
        socket.write_all(&response.to_bytes()).await?;

        // it the command was QUIT, close the connection and return
        if matches!(cmd, Command::Quit) {
            return Ok(());
        }
    }
}