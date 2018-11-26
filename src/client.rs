use errors::Error;
use std::thread;
use std::net::{TcpStream, Shutdown};
use std::os::unix::net::UnixStream;
use std::sync::{mpsc, Arc, Mutex};
use stream::ConStream;
use message::{Msg, MsgName, MsgReading, MSG_REQ, MSG_WITH_BODY};
use utils;

pub type HandlerFunc<T> = fn(msg: Msg, state: SharedState<T>, ctx: Arc<Mutex<T>>);
pub type SharedState<T> = Arc<Mutex<State<T>>>;
pub type OptBody = Option<Vec<u8>>;

#[derive(Debug)]
pub struct Handler<T> {
    func: Option<HandlerFunc<T>>,
    ans: Option<mpsc::Sender<OptBody>>,
    once: bool,
    called: bool,
    msg_id: Option<u128>,
    msg_name: Option<String>,
    client_id: Option<String>,
}

#[derive(Debug)]
pub struct State<T> {
    pub handlers: Vec<Handler<T>>,
}

#[derive(Debug)]
pub struct Client<T> {
    pub state: Arc<Mutex<State<T>>>,
    pub ctx: Arc<Mutex<T>>,
    id: Option<String>,
    stream: ConStream,
}

impl<T: Sync + Send + 'static> Client<T> {
    /// Connect to server
    pub fn connect(address: &str, ctx: T, name: Option<&str>) -> Result<Client<T>, Error> {
        // Connect
        let stream = if address.starts_with("/") && address.ends_with(".sock") {
            let stream = UnixStream::connect(address)?;
            ConStream::new_unix(stream)
        } else {
            let stream = TcpStream::connect(address)?;
            ConStream::new_tcp(stream)
        };

        let state = State {
            handlers: Vec::with_capacity(5),
        };

        let mut cloned_stream = stream.try_clone()?;
        let mut instance = Client {
            state: Arc::new(Mutex::new(state)),
            ctx: Arc::new(Mutex::new(ctx)),
            id: None,
            stream: stream,
        };
        instance.handshake(&mut cloned_stream, name);

        let mux_state = instance.state.clone();
        let mux_ctx = instance.ctx.clone();
        thread::spawn(move || {
            Msg::read(&mut cloned_stream, |msg| {
                let msg = match Client::<T>::parse_msg(msg) {
                    Ok(msg) => msg,
                    Err(_) => return MsgReading::Stop,
                };

                let msg_id = msg.id;
                let msg_name = msg.name.clone();
                let msg_body = msg.body.clone();
                let mut state = mux_state.lock().unwrap();
                for h in  state.handlers.iter_mut() {
                    let mut matched = true;
                    if let Some(ref id) = h.msg_id {
                        matched = matched && *id == msg_id;
                    }
                    if let Some(ref name) = h.msg_name {
                        matched = matched && *name == msg_name;
                    }
                    if matched {
                        if h.once {
                            if !h.called {
                                if let Some(ref mut ans) = h.ans {
                                    ans.send(msg_body.clone()).unwrap();
                                }
                                if let Some(f) = h.func {
                                    f(msg.clone(), mux_state.clone(), mux_ctx.clone());
                                }
                                h.called = true
                            }
                        } else {
                            if let Some(ref mut ans) = h.ans {
                                ans.send(msg_body.clone()).unwrap();
                            }
                            if let Some(f) = h.func {
                                f(msg.clone(), mux_state.clone(), mux_ctx.clone());
                            }
                        }
                    }
                }

                MsgReading::Continue
            });
        });

        Ok(instance)
    }

    /// Send message to server
    pub fn send(&mut self, name: &str, body: OptBody) {
        let id = utils::bid();
        let meta = match body {
            Some(_) => MSG_WITH_BODY,
            None => 0,
        };

        Msg::write(&mut self.stream, &id, &[meta], name.as_bytes(), &body);
    }

    /// Send request to server
    pub fn req(&mut self, name: &str, body: OptBody, ans: mpsc::Sender<OptBody>) {
        let id = utils::bid();
        let meta = match body {
            Some(_) => MSG_REQ | MSG_WITH_BODY,
            None => MSG_REQ,
        };

        let mut state = self.state.lock().unwrap();
        state.handlers.push(Handler {
            func: None,
            ans: Some(ans.clone()),
            once: true,
            called: false,
            msg_id: Some(utils::bid_to_u128(&id)),
            msg_name: Some(name.to_string()),
            client_id: None,
        });
        drop(state);

        Msg::write(&mut self.stream, &id, &[meta], name.as_bytes(), &body);
    }

    /// Subscribe.
    pub fn on(&mut self, msg_name: MsgName, func: HandlerFunc<T>) {
        let mut state = self.state.lock().unwrap();
        let msg_name = match msg_name {
            MsgName::Is(name) => Some(name.to_string()),
            MsgName::Any => None,
        };

        state.handlers.push(Handler {
            func: Some(func),
            ans: None,
            once: false,
            called: false,
            msg_id: None,
            msg_name: msg_name,
            client_id: None,
        });
        drop(state);
    }

    /// Subscribe on next message.
    pub fn once(&mut self, msg_name: MsgName, func: HandlerFunc<T>) {
        let mut state = self.state.lock().unwrap();
        let msg_name = match msg_name {
            MsgName::Is(name) => Some(name.to_string()),
            MsgName::Any => None,
        };

        state.handlers.push(Handler {
            func: Some(func),
            ans: None,
            once: true,
            called: false,
            msg_id: None,
            msg_name: msg_name,
            client_id: None,
        });
        drop(state);
    }

    /// Try to disconnect from server.
    pub fn disconnect(&mut self) -> Result<(), Error> {
        self.id = None;
        self.stream.shutdown(Shutdown::Both)?;
        Ok(())
    }

    /// Handshake with server. Will block thread.
    fn handshake(&mut self, stream: &mut ConStream, name: Option<&str>) {
        let id = utils::bid();
        let name_bin = "handshake".as_bytes();
        let body = match name {
            Some(name) => Some(Vec::from(name)),
            None => None,
        };

        Msg::write(stream, &id, &[MSG_REQ | MSG_WITH_BODY], name_bin, &body);
        Msg::read(stream, |msg| {
            if let Ok(msg) = Client::<T>::parse_msg(msg) {
                // Skip non-handshake response
                if msg.name != "handshake" { return MsgReading::Continue; }

                self.id = match msg.body {
                    Some(b) => Some(String::from_utf8_lossy(&b).to_string()),
                    None => None,
                };
                println!(" → Handshaked! {:?}", self.id);
            }
            MsgReading::Stop
        });
    }

    /// Parse binary clice to message.
    fn parse_msg(msg: &[u8]) -> Result<Msg, Error> {
        // Id, meta, name
        let id: u128 = utils::bid_to_u128(&msg[0..12]);
        let name_end: usize = msg[13] as usize + 14;
        let name = String::from_utf8_lossy(&msg[14..name_end]).to_string();

        // Body
        let mut body: Option<Vec<u8>> = None;
        if msg.len() > name_end {
            let body_len = utils::bytes_to_u64(&msg[name_end..name_end + 8]) as usize;
            let body_start = name_end + 8;
            let body_end = body_start + body_len;
            body = Some(Vec::from(&msg[body_start..body_end]));
        }

        Ok(Msg {
            client: "".to_string(),
            id: id,
            req: false,
            name: name,
            body: body,
            ans_tx: None,
        })
    }
}
