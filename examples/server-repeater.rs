extern crate con;

use con::ClientName;
use con::Error;
use con::MsgName;
use con::Server;
use std::sync::{Arc, Mutex};

fn repeat<T>(
    msg: con::Msg,
    state: con::server::SharedState<T>,
    _ctx: Arc<Mutex<u64>>,
) -> Option<Vec<u8>> {
    let state = state.lock().unwrap();
    let client = state
        .clients
        .iter()
        .find(|c| c.id == msg.client)
        .and_then(|ref c| c.name.clone())
        .unwrap_or("<notfoundclient>".to_string());

    if let Some(body) = msg.body {
        let input = String::from_utf8_lossy(&body).to_string();
        let cl = state.clients.len();
        println!(" → Clients: {:?}, {:?} - {:?}", cl, client, input);
        return Some(input.repeat(3).into_bytes());
    }
    None
}

fn main() -> Result<(), Error> {
    let ctx = Arc::new(Mutex::new(0u64));
    let mut server = Server::new(ctx.clone());

    // Handle request
    server.on(ClientName::Any, MsgName::Is("repeat"), repeat)?;

    // Start listening in other threads
    server.listen("127.0.0.1:1234")?;
    // server.listen("/tmp/con-examples.sock")?;
    // server.listen_all(&["/tmp/con-examples.sock", "127.0.0.1:1234"])?;

    Ok(())
}
