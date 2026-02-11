
# Rusdis a simple Redis "clone"


## Overview

This project is a simplified Redis-like server in Rust, built primarily using the tokio asynchronous runtime. 

The goal was to build a functional, but basic, key-value store that communicates using the Redis Serialization Protocol (RESP).

## Features
* **Network server**: asynchronously accepts multiple client connections
* **RESP Protocol Handling**: Parses incoming commands (```SET```, ```GET```, ```DEL```, ```AUTH```, ```QUIT```) from RESP arrays and serializes responses back to RESP.
* **In memory Key-Value store**: Stores string keys and binary safe values ```(vec<u8>)```
* **Authentication**: Basic Authentication trough a hardcoded password, clients must authenticate before executing other commands, using ```AUTH <password>```
* **Concurrent client handling**: Uses ```tokio::spawn``` to handle each client connection on its own asynchronous task, enabling the hability to manage multiple simultaneous clients.
