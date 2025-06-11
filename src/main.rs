use std::{
    collections::HashMap, // in memory key-value store
    sync::{Arc, Mutex},   // Arc for shared ownership, Mutex for mutual exclusion
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt}, // Asynchronous read and write operations on streams
    net::{TcpListener, TcpStream},     // TcpListener to accept incoming connections, TcpStream for individual connections
};

#[tokio::main] // mark this as the main function for a tokio runtime
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // create in memory HashMap to store key-value pairs with arc<mutex for safe, shared and mutable access to it
    let db: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::new()));

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
    db: Arc<Mutex<HashMap<String, String>>>,
) -> Result<(), Box<dyn std::error::Error>> {
    // buffer to read incoming data from the client
    // read up to 1024 bytes at a time
    let mut buf = vec![0; 1024];

    // loop indefinitely to read commands
    loop {
        // read bytes from the TCP stream into the buffer
        // "read" returns the number of bytes read. If 0, the client has disconnected
        let n = socket.read(&mut buf).await?;

        if n == 0 {
            // client disconnected
            return Ok(());
        }

        // convert the received bytes into a string assuming UTF.8 input
        let command_str = String::from_utf8_lossy(&buf[..n]).trim().to_string();
        println!("Received command: '{}'", command_str);

        // simple parsing
        let parts: Vec<&str> = command_str.split_whitespace().collect();

        // initialize a response string
        let response: String;

        // match 1st split to available commands
        match parts.get(0).map(|s| s.to_ascii_uppercase()) {
            Some(cmd) => {
                match cmd.as_str(){                
                    "SET" => {
                        // handle set command: Set key value
                        if parts.len() >= 3 {
                            let key = parts[1].to_string();
                            // join remaining parts after key parts(1) as value possibly separated by spaces 
                            let value = parts[2..].join(" ");

                            // acquire a lock on the Mutex to safely access the HashMap
                            // this blocks other threads/tasks from writing to the HashMap until lock is released
                            let mut db_locked = db.lock().unwrap(); // `unwrap()` is "safe" here for simplicity
                            db_locked.insert(key, value);
                            response = "OK\n".to_string();
                        } else {
                            response = "ERR wrong number of arguments for 'SET' command\n".to_string();
                        }
                    }
                    "GET" => {
                        // handle get command: GET key
                        if parts.len() == 2 {
                            let key = parts[1].to_string();

                            // acquire lock again
                            let db_locked = db.lock().unwrap();
                            response = match db_locked.get(&key) {
                                Some(value) => format!("{}\n", value), // key found return value
                                None => "(nil)\n".to_string(),        // key not found return nil
                            };
                        } else {
                            response = "ERR wrong number of arguments for 'GET' command\n".to_string();
                        }
                    }
                    "DEL" => {
                        // handle del command: DEL key
                        if parts.len() == 2 {
                            let key = parts[1].to_string();

                            // acquire lock
                            let mut db_locked = db.lock().unwrap();
                            response = match db_locked.remove(&key) {
                                Some(_) => "OK\n".to_string(), // key exists and is removed
                                None => "(nil)\n".to_string(), // key not found
                            };
                        } else {
                            response = "ERR wrong number of arguments for 'DEL' command\n".to_string();
                        }
                    }
                    "QUIT" => {
                        // quit command to close the conn
                        response = "Connection will be closed shortly!\n".to_string();
                        socket.write_all(response.as_bytes()).await?;
                        return Ok(()); // exit the loop and terminate the client handler task
                    }
                    "LIST" => {
                        // list command to list available commands
                        response = "Available commands are:\n SET <key> <value>\n GET <key>\n DEL <key>\n QUIT (to close connection)\n".to_string();
                    }
                    _ => {
                        // handle unknown commands
                        response = "ERR unknown command\n Use LIST to see available commands\n".to_string();
                    }
                }
            },
            None => {
                // handle empty command string
                response = "ERR empty command\n".to_string();
            }
        }
            

        // write generated response back to the client
        socket.write_all(response.as_bytes()).await?;
    }
}

