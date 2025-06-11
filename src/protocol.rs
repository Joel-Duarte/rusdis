use tokio::io::AsyncReadExt;

// represent the different types of values whitin RESP
#[derive(Debug, PartialEq, Clone)]
pub enum RespValue {
    SimpleString(String),
    Error(String),
    Integer(i64),
    BulkString(Vec<u8>),
    Array(Vec<RespValue>),
    Null, // represents a null bulk string ($-1\r\n) or null array
}

impl RespValue {
    // serializes a `RespValue` into its byte representation
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            RespValue::SimpleString(s) => format!("+{}\r\n", s).into_bytes(),
            RespValue::Error(s) => format!("-{}\r\n", s).into_bytes(),
            RespValue::Integer(i) => format!(":{}\r\n", i).into_bytes(),
            RespValue::BulkString(b) => {
                let mut bytes = Vec::new();
                bytes.extend_from_slice(format!("${}\r\n", b.len()).as_bytes());
                bytes.extend_from_slice(b);
                bytes.extend_from_slice(b"\r\n");
                bytes
            }
            RespValue::Array(arr) => {
                let mut bytes = Vec::new();
                bytes.extend_from_slice(format!("*{}\r\n", arr.len()).as_bytes());
                for item in arr {
                    bytes.extend_from_slice(&item.to_bytes());
                }
                bytes
            }
            RespValue::Null => b"$-1\r\n".to_vec(),
        }
    }

    // attempt to parse bytes from an `AsyncRead` (like a `TcpStream`) into a `RespValue`.
    // Simplified parser implementation
    pub async fn from_stream(stream: &mut (impl AsyncReadExt + Unpin)) -> Result<Option<RespValue>, Box<dyn std::error::Error + Send + Sync >> { // added Send + Sync for error handling across threads
        let mut buf = Vec::new();
        let mut temp_byte = [0; 1];

        // read the first byte to determine the RESP type
        let n = stream.read_exact(&mut temp_byte).await?;
        if n == 0 {
            return Ok(None); // end of stream client disconnected
        }

        let prefix = temp_byte[0];

        // read until CRLF (CRLF is \r\n) for the length/value part, or for the whole simplestring/rrror/integer
        loop {
            let n = stream.read_exact(&mut temp_byte).await?;
            if n == 0 {
                return Err("Unexpected end of stream while reading RESP content".into());
            }
            buf.push(temp_byte[0]);
            if buf.len() >= 2 && &buf[buf.len() - 2..] == b"\r\n" {
                break;
            }
        }
        // remove the trailing CRLF
        let line = String::from_utf8(buf[0..buf.len() - 2].to_vec())?;

        match prefix {
            b'+' => Ok(Some(RespValue::SimpleString(line))),
            b'-' => Ok(Some(RespValue::Error(line))),
            b':' => Ok(Some(RespValue::Integer(line.parse()?))),
            b'$' => {
                let len: i64 = line.parse()?;
                if len == -1 {
                    Ok(Some(RespValue::Null))
                } else {
                    let mut data_buf = vec![0; len as usize];
                    stream.read_exact(&mut data_buf).await?;
                    // read the trailing CRLF or bulk string
                    stream.read_exact(&mut [0; 2]).await?;
                    Ok(Some(RespValue::BulkString(data_buf)))
                }
            }
            b'*' => {
                let num_elements: i64 = line.parse()?;
                if num_elements == -1 {
                    Ok(Some(RespValue::Null))
                } else {
                    let mut elements = Vec::with_capacity(num_elements as usize);
                    for _ in 0..num_elements {
                        if let Some(element) = Box::pin(RespValue::from_stream(stream)).await? {
                            elements.push(element);
                        } else {
                            return Err("Unexpected end of stream while reading array elements".into());
                        }
                    }
                    Ok(Some(RespValue::Array(elements)))
                }
            }
            _ => Err(format!("Unknown RESP prefix: {}", prefix as char).into()),
        }
    }
}