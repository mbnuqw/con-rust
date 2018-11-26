use errors::Error;
use message::{Msg, MsgName, MsgReading, MSG_WITH_BODY};
use std::fs;
use std::io;
use std::net::{Shutdown, TcpListener};
use std::os::unix::net::UnixListener;
use std::sync::{Arc, Mutex};
use std::thread;
use stream::ConStream;
use utils;

pub type SharedState<T> = Arc<Mutex<State<T>>>;
pub type HandlerFunc<T> = fn(Msg, SharedState<T>, Arc<Mutex<T>>) -> Option<Vec<u8>>;

pub struct Handler<T> {
    func: HandlerFunc<T>,
    once: bool,
    called: bool,
    msg_id: Option<u128>,
    msg_name: Option<String>,
    client_id: Option<String>,
    client_name: Option<String>,
}

pub enum ClientName<'a> {
    Any,
    Is(&'a str),
}

#[derive(Debug)]
pub struct ConnectedClient {
    pub id: String,
    pub name: Option<String>,
    pub stream: ConStream,
}

impl ConnectedClient {
    pub fn new(name: Option<&str>, stream: ConStream) -> Self {
        let name = match name {
            Some(name) => Some(name.to_string()),
            None => None,
        };

        ConnectedClient {
            id: utils::uid(),
            name: name,
            stream: stream,
        }
    }
}

pub struct State<T> {
    pub clients: Vec<ConnectedClient>,
    pub handlers: Vec<Handler<T>>,
}

pub struct Server<T> {
    pub state: SharedState<T>,
    pub ctx: Arc<Mutex<T>>,
}

impl<T: Send + Sync + 'static> Server<T> {
    ///  Construct new server
    pub fn new(ctx: Arc<Mutex<T>>) -> Server<T> {
        // Setup handlers
        let mut handlers = Vec::with_capacity(5);
        handlers.push(Handler {
            func: Server::<T>::handle_handshake,
            once: false,
            called: false,
            msg_id: None,
            msg_name: Some("handshake".to_string()),
            client_id: None,
            client_name: None,
        });

        // Create initial struct
        let state = State {
            clients: Vec::new(),
            handlers: handlers,
        };
        let state = Arc::new(Mutex::new(state));

        Server { state, ctx }
    }

    ///  Listen given address
    pub fn listen(&mut self, address: &str) -> Result<(), Error> {
        if address.starts_with("/") && address.ends_with(".sock") {
            Server::<T>::handle_unix_clients(address, self.state.clone(), self.ctx.clone())?;
        } else {
            Server::<T>::handle_tcp_clients(address, self.state.clone(), self.ctx.clone())?;
        };

        Ok(())
    }

    /// Listen all addresses in separated threads.
    pub fn listen_all(&mut self, addresses: &'static [&str]) -> Result<(), Error> {
        addresses.iter().for_each(|address| {
            let state = self.state.clone();
            let ctx = self.ctx.clone();
            thread::spawn(move || {
                if address.starts_with("/") && address.ends_with(".sock") {
                    Server::<T>::handle_unix_clients(address, state, ctx)
                        .expect("cannot listen unix socket");
                } else {
                    Server::<T>::handle_tcp_clients(address, state, ctx)
                        .expect("cannot listen tcp socket");
                };
            });
        });

        Ok(())
    }

    /// Add message handler
    pub fn on(&mut self, client_name: ClientName, msg_name: MsgName, h: HandlerFunc<T>) -> Result<(), Error> {
        self.subs(client_name, msg_name, false, h)
    }

    /// Add message handler
    pub fn once(&mut self, client_name: ClientName, msg_name: MsgName, h: HandlerFunc<T>) -> Result<(), Error> {
        self.subs(client_name, msg_name, true, h)
    }

    /// Broadcast message to all connected clients
    pub fn broadcast(
        state: &SharedState<T>,
        msg_name: &str,
        body: Option<Vec<u8>>,
    ) -> Result<(), Error> {
        let state = match state.lock() {
            Ok(s) => s,
            Err(_) => return Err(Error::Mutex),
        };

        let msg_id = utils::bid();
        let msg_meta = if body.is_none() { 0u8 } else { MSG_WITH_BODY };
        let name_bin = msg_name.as_bytes();

        for client in state.clients.iter() {
            let mut s = client.stream.try_clone()?;
            Msg::write(&mut s, &msg_id, &[msg_meta], name_bin, &body);
        }

        Ok(())
    }

    /// Send message to client
    pub fn send(
        state: &SharedState<T>,
        client_name: &str,
        msg_name: &str,
        body: Option<Vec<u8>>,
    ) -> Result<(), Error> {
        let state = match state.lock() {
            Ok(s) => s,
            Err(_) => return Err(Error::Mutex),
        };

        let msg_id = utils::bid();
        let msg_meta = if body.is_none() { 0u8 } else { MSG_WITH_BODY };
        let name_bin = msg_name.as_bytes();

        let client = state.clients.iter().find(|c| match c.name {
            Some(ref name) => name == client_name,
            None => false,
        });

        if let Some(client) = client {
            let mut s = client.stream.try_clone()?;
            Msg::write(&mut s, &msg_id, &[msg_meta], name_bin, &body);
        }

        Ok(())
    }

    /// Disconnect peer
    pub fn disconnect(state: &SharedState<T>, client: &str) -> Result<(), Error> {
        let state = match state.lock() {
            Ok(s) => s,
            Err(_) => return Err(Error::Mutex),
        };

        let maybe_i = state.clients.iter().position(|c| match c.name {
            Some(ref name) => c.id == client || name == client,
            None => c.id == client,
        });

        let i = match maybe_i {
            Some(i) => i,
            None => return Err(Error::ClientNotFound),
        };

        state.clients[i].stream.shutdown(Shutdown::Both)?;

        Ok(())
    }

    /// Start listening tcp stream
    fn handle_tcp_clients(
        address: &str,
        state: SharedState<T>,
        ctx: Arc<Mutex<T>>,
    ) -> Result<(), Error> {
        let listener = TcpListener::bind(address)?;
        let state = state.clone();

        // Handle incoming connections
        for conn in listener.incoming() {
            println!(" → Incomming connection");
            match conn {
                Ok(s) => {
                    let stream = ConStream::new_tcp(s);
                    Server::<T>::handle_client(state.clone(), stream, ctx.clone())?;
                }
                Err(_) => break,
            }
        }
        Ok(())
    }

    /// Start listening unix stream.
    fn handle_unix_clients(
        addr: &str,
        state: SharedState<T>,
        ctx: Arc<Mutex<T>>,
    ) -> Result<(), Error> {
        let listener = Server::<T>::open_sock(addr)?;
        let state = state.clone();

        // Handle incoming connections
        for conn in listener.incoming() {
            println!(" → Incomming connection");
            match conn {
                Ok(s) => {
                    let stream = ConStream::new_unix(s);
                    Server::handle_client(state.clone(), stream, ctx.clone())?;
                }
                Err(_) => break,
            }
        }
        Ok(())
    }

    /// Handle new client in separated thread.
    fn handle_client(
        state: SharedState<T>,
        stream: ConStream,
        ctx: Arc<Mutex<T>>,
    ) -> Result<(), Error> {
        // Create client
        let client = ConnectedClient::new(None, stream.try_clone()?);
        let cli_id = client.id.clone();

        // Read stream in new thread
        let state_clone = state.clone();
        thread::spawn(move || {
            Server::handle_messages(&cli_id, state_clone, stream, ctx).unwrap();
        });

        // Add new client to server state
        let mut locked_state = match state.lock() {
            Ok(s) => s,
            Err(_) => return Err(Error::Mutex),
        };
        locked_state.clients.push(client);
        drop(locked_state);

        Ok(())
    }

    /// Handle messages from connected client.
    fn handle_messages(
        client_id: &str,
        state: SharedState<T>,
        mut stream: ConStream,
        ctx: Arc<Mutex<T>>,
    ) -> Result<(), Error> {
        Msg::read(&mut stream, |msg| {
            match Server::handle_message(&client_id, state.clone(), msg, ctx.clone()) {
                Ok(_) => MsgReading::Continue,
                Err(_) => MsgReading::Stop,
            }
        });

        // Handle client disconnecting
        let mut state = match state.lock() {
            Ok(s) => s,
            Err(_) => return Err(Error::Mutex),
        };

        let maybe_client_index = state.clients.iter().position(|c| c.id == client_id);
        if let Some(client_index) = maybe_client_index {
            state.clients.remove(client_index);
        }

        Ok(())
    }

    /// Find message handler and call it.
    fn handle_message(
        client_id: &str,
        state: SharedState<T>,
        msg_buff: &[u8],
        ctx: Arc<Mutex<T>>,
    ) -> Result<(), Error> {
        let msg = Msg::from_bytes(&msg_buff, &client_id.clone());
        let state_clone = state.clone();
        let mut locked_state = match state.lock() {
            Ok(s) => s,
            Err(_) => return Err(Error::Mutex),
        };

        // Find handler
        for h in locked_state.handlers.iter_mut() {
            let mut matched = true;
            if let Some(ref msg_id) = h.msg_id {
                matched = matched && *msg_id == msg.id;
            }
            if let Some(ref msg_name) = h.msg_name {
                matched = matched && *msg_name == msg.name;
            }
            if !h.client_name.is_none() {
                matched = matched && match h.client_id {
                    Some(ref cli_id) => *cli_id == client_id,
                    None => false,
                };
            }
            if matched {
                let mut func = h.func;
                if h.once {
                    if !h.called {
                        Server::call_handler(func, msg.clone(), state_clone.clone(), ctx.clone());
                        h.called = true
                    }
                } else {
                    Server::call_handler(h.func, msg.clone(), state_clone.clone(), ctx.clone());
                }
            }
        }

        Ok(())
    }

    /// Call message handler in separated thread.
    fn call_handler(h: HandlerFunc<T>, msg: Msg, state: SharedState<T>, ctx: Arc<Mutex<T>>) {
        let is_req = msg.req;
        let msg_id = msg.id;
        let msg_name = msg.name.clone();
        let msg_client = msg.client.clone();

        thread::spawn(move || {
            let ans = (h)(msg, state.clone(), ctx);
            if !is_req {
                return;
            }

            let locked_state = state.lock().unwrap();
            let mut client = locked_state.clients.iter().find(|c| c.id == msg_client);
            if let Some(ref mut client) = client {
                let mut stream = client.stream.try_clone().unwrap();
                Msg::write(
                    &mut stream,
                    &utils::u128_to_bytes(msg_id),
                    &[MSG_WITH_BODY],
                    msg_name.as_bytes(),
                    &ans,
                );
            }
        });
    }

    /// Open socket.
    fn open_sock(path: &str) -> Result<UnixListener, Error> {
        match UnixListener::bind(path) {
            Ok(l) => Ok(l),
            Err(err) => {
                // Socket exists -> remove it and open new
                if err.kind() == io::ErrorKind::AddrInUse {
                    fs::remove_file(path)?;
                    Ok(UnixListener::bind(path)?)
                } else {
                    Err(Error::from(err))
                }
            }
        }
    }

    /// Subscribes on some message.
    fn subs(
        &mut self,
        client_name: ClientName,
        msg_name: MsgName,
        once: bool,
        h: HandlerFunc<T>,
    ) -> Result<(), Error> {
        let mut state = match self.state.lock() {
            Ok(s) => s,
            Err(_) => return Err(Error::Mutex),
        };

        let client_id = match client_name {
            ClientName::Is(client_name) => {
                let maybe_client = state.clients.iter().find(|c| match c.name {
                    Some(ref name) => name == client_name,
                    None => false,
                });
                match maybe_client {
                    Some(client) => Some(client.id.clone()),
                    None => None,
                }
            }
            ClientName::Any => None,
        };

        let client_name = match client_name {
            ClientName::Is(client_name) => Some(client_name.to_string()),
            ClientName::Any => None,
        };

        let msg_name = match msg_name {
            MsgName::Any => None,
            MsgName::Is(val) => Some(val.to_string()),
        };

        state.handlers.push(Handler {
            func: h,
            once: once,
            called: false,
            msg_id: None,
            msg_name: msg_name,
            client_id: client_id,
            client_name: client_name,
        });

        Ok(())
    }

    /// Handle handshake.
    fn handle_handshake(msg: Msg, state: SharedState<T>, _ctx: Arc<Mutex<T>>) -> Option<Vec<u8>> {
        // Update client name
        if let Some(ref body) = msg.body {
            let mut state = state.lock().unwrap();
            let new_name = String::from_utf8_lossy(body).to_string();
            let client_id: &str = &msg.client;
            {
                let maybe_client = state.clients.iter_mut().find(|c| c.id == client_id);
                if let Some(client) = maybe_client {
                    client.name = Some(new_name.clone());
                }
            }

            // Update handlers
            for handler in state.handlers.iter_mut() {
                let matched = match handler.client_name {
                    Some(ref name) => *name == new_name,
                    None => false,
                };
                if matched {
                    handler.client_id = Some(client_id.to_string());
                }
            }
        }
        Some(Vec::from(msg.client))
    }
}

// -----------------------------
// --- --- --- Tests --- --- ---
// -----------------------------
#[cfg(test)]
mod tests {
    use std::sync::{Mutex, Arc};
    use server::*;

    #[test]
    fn adding_new_handler() {
        let mut server = Server::new(Arc::new(Mutex::new(())));

        // Only one handler - handshake
        {
            let state = server.state.lock().unwrap();
            assert_eq!(state.handlers.len(), 1);
        }

        // Add handler
        server.on(ClientName::Any, MsgName::Any, |_, _, _| None).unwrap();
        {
            let state = server.state.lock().unwrap();
            assert_eq!(state.handlers.len(), 2);
        }
    }
}