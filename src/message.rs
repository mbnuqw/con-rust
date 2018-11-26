use std::io::{Read, Write};
use std::sync::mpsc;
use utils;

// Msg meta flags
pub static MSG_WITH_BODY: u8   = 0b1_0_0_00000;
pub static MSG_REQ: u8         = 0b0_1_0_00000;

pub enum MsgName<'a> {
    Any,
    Is(&'a str),
}

pub enum MsgReading {
    Continue,
    Stop,
}

#[derive(Debug, Clone)]
pub struct Msg {
    pub client: String,
    pub id: u128,
    pub req: bool,
    pub name: String,
    pub body: Option<Vec<u8>>,
    pub ans_tx: Option<mpsc::Sender<Option<Vec<u8>>>>,
}

impl Msg {
    /// Create new message with required fields.
    pub fn new(client: &str, id: u128, meta: u8, name: &str) -> Self {
        Msg {
            id: id,
            req: (meta & MSG_REQ) == MSG_REQ,
            name: name.to_string(),
            client: client.to_string(),
            body: None,
            ans_tx: None,
        }        
    }

    /// Create new message from bytes.
    pub fn from_bytes(bin: &[u8], client_id: &str) -> Self {
        // Id, meta, name
        let id: u128 = utils::bid_to_u128(&bin[0..12]);
        let meta = bin[12];
        let name_end: usize = bin[13] as usize + 14;
        let name = String::from_utf8_lossy(&bin[14..name_end]).to_string();

        // Body
        let mut body: Option<Vec<u8>> = None;
        if bin.len() > name_end {
            let body_len = utils::bytes_to_u64(&bin[name_end..name_end + 8]) as usize;
            let body_start = name_end + 8;
            let body_end = body_start + body_len;
            body = Some(Vec::from(&bin[body_start..body_end]));
        }

        Msg {
            id: id,
            req: (meta & MSG_REQ) == MSG_REQ,
            name: name,
            client: client_id.to_string(),
            body: body,
            ans_tx: None,
        }
    }

    /// Set body.
    pub fn with_body(mut self, body: &[u8]) -> Self {
        self.body = Some(Vec::from(body));
        self
    }

    /// Set body from string.
    pub fn with_str_body(mut self, body: &str) -> Self {
        self.body = Some(Vec::from(body));
        self
    }

    /// Create binary message.
    pub fn raw(id: u128, meta: u8, name: &str, body: Option<Vec<u8>>) -> Vec<u8> {
        let body_len = match body {
            Some(ref b) => b.len(),
            None => 0,
        };
        let mut msg = Vec::with_capacity(22 + name.len() + body_len);

        msg.extend_from_slice(&utils::u128_to_bytes(id));
        msg.push(meta);
        msg.extend_from_slice(&[name.len() as u8]);
        msg.extend_from_slice(name.as_bytes());
        if let Some(ref b) = body {
            msg.extend_from_slice(&utils::u64_to_bytes(b.len() as u64));
            msg.extend_from_slice(b);
        }

        msg
    }

    /// Write message to stream.
    pub fn write<S: Write>(
        stream: &mut S,
        id: &[u8],
        meta: &[u8],
        name: &[u8],
        body: &Option<Vec<u8>>,
    ) {
        let body_len = match body {
            Some(b) => b.len(),
            None => 0,
        };
        let mut msg_buff = Vec::with_capacity(22 + name.len() + body_len);
        
        msg_buff.extend_from_slice(id);
        msg_buff.extend_from_slice(meta);
        msg_buff.extend_from_slice(&[name.len() as u8]);
        msg_buff.extend_from_slice(name);
        if let Some(b) = body {
            msg_buff.extend_from_slice(&utils::u64_to_bytes(b.len() as u64));
            msg_buff.extend_from_slice(b);
        }
        stream.write(&msg_buff).expect("Cannot flush buffer");
        stream.flush().expect("Cannot flush buffer");
    }

    /// Start reading stream.
    pub fn read<F>(stream: &mut Read, mut f: F)
    where
        F: FnMut(&[u8]) -> MsgReading,
    {
        const BUFF_SIZE: usize = 2048;
        const CHUNK_SIZE: usize = 1024;

        let mut read_buff = [0; CHUNK_SIZE];
        let mut msg_buff = Vec::with_capacity(BUFF_SIZE);
        let mut read_len: usize = 0;
        let mut meta: u8 = 0;
        let mut with_body: bool = false;
        let mut name_len: u8 = 0;
        let mut name_end: usize = 0;
        let mut body_len: u64 = 0;
        let mut msg_end: usize = 0;
        loop {
            if let Ok(n) = stream.read(&mut read_buff) {
                if n == 0 {
                    break;
                }

                msg_buff.extend_from_slice(&read_buff[0..n]);
                read_len += n;

                while msg_buff.len() > 0 {
                    // Meta
                    if meta == 0 && read_len >= 13 {
                        meta = msg_buff[12];
                        with_body = (meta & MSG_WITH_BODY) != 0;
                    }

                    // Name len
                    if name_len == 0 && read_len >= 14 {
                        name_len = msg_buff[13];
                        name_end = (14 + name_len) as usize;
                        if !with_body {
                            msg_end = name_end
                        };
                    }

                    // Body len
                    if with_body && name_len > 0 && body_len == 0 && read_len >= name_end + 8 {
                        body_len = utils::bytes_to_u64(&msg_buff[name_end..name_end + 8]);
                        msg_end = name_end as usize + 8 + body_len as usize;
                    }

                    // Got full message
                    if msg_end > 0 && read_len >= msg_end {
                        // Handle message
                        match f(&msg_buff[..msg_end]) {
                            MsgReading::Continue => (),
                            MsgReading::Stop => return,
                        }

                        // Clean up
                        meta = 0;
                        with_body = false;
                        name_len = 0;
                        name_end = 0;
                        body_len = 0;
                        if read_len == msg_end {
                            msg_buff.clear();
                            read_len = 0;
                            msg_end = 0;
                            // reading: exact full message
                            break;
                        } else {
                            msg_buff.drain(..msg_end);
                            read_len -= msg_end;
                            msg_end = 0;
                            // reading: full message with extra stuff
                            continue;
                        }
                    } else {
                        // reading: not enough data
                        break;
                    }
                }
            } else {
                // Reading error
                break;
            }
        }

        // Return disconnection message
        f(&Msg::raw(0, 0, "disconnect", None));
    }
}
