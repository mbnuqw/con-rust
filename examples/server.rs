extern crate con;

use con::ClientName;
use con::Error;
use con::MsgName;
use con::Server;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time;

fn main() -> Result<(), Error> {
    let ctx = Arc::new(Mutex::new(0u64));
    let mut server = Server::new(ctx.clone());
    let server_state = server.state.clone();

    // Subscribe: all clients - msg 'msg-A'
    server.on(ClientName::Any, MsgName::Is("msg-A"), |msg, state, ctx| {
        let state = state.lock().unwrap();
        let mut ctx = ctx.lock().unwrap();
        let client_name = match state.clients.iter().find(|c| c.id == msg.client) {
            Some(c) => c.name.clone(),
            None => None,
        };

        println!(" → {:?} Client: {:?}, msg: 'msg-A'", *ctx, client_name);
        if let Some(body) = msg.body {
            println!("      with body: {:?}", String::from_utf8_lossy(&body));
        }
        *ctx += 1;
        None
    })?;

    // Subscribe: client 'client-a' - msg 'msg-A'
    server.on(
        ClientName::Is("client-a"),
        MsgName::Is("msg-A"),
        |_msg, _state, ctx| {
            let mut ctx = ctx.lock().unwrap();
            println!(" → {:?} Client: 'client-a', msg: 'msg-A'", *ctx);
            *ctx += 1;
            None
        },
    )?;

    // Subscribe for next msg: client 'client-b' - msg 'msg-A'
    server.once(
        ClientName::Is("client-b"),
        MsgName::Is("msg-A"),
        |_msg, _state, ctx| {
            let mut ctx = ctx.lock().unwrap();
            println!(" → {:?} Once Client: 'client-b', msg: 'msg-A'", *ctx);
            *ctx += 1;
            None
        },
    )?;

    // Subscribe: client 'client-b' - all messages
    server.on(
        ClientName::Is("client-b"),
        MsgName::Any,
        |msg, _state, ctx| {
            let mut ctx = ctx.lock().unwrap();
            println!(" → {:?} Client: 'client-b', msg: {:?}", *ctx, msg.name);
            *ctx += 1;
            None
        },
    )?;

    // Handle request
    server.on(
        ClientName::Is("client-a"),
        MsgName::Is("repeat"),
        |msg, _state, ctx| {
            let mut ctx = ctx.lock().unwrap();
            println!(" → {:?} Client: 'client-a', msg: 'repeat'", *ctx);
            *ctx += 1;
            if let Some(body) = msg.body {
                let input = String::from_utf8_lossy(&body).to_string();
                println!(" → REPEATED {:?} - {:?}", input.repeat(3), msg.id);
                return Some(input.repeat(3).into_bytes());
            }
            None
        },
    )?;

    server.on(
        ClientName::Any,
        MsgName::Is("disconnect"),
        |_msg, _state, _ctx| {
            println!(" → Disconnect!");
            None
        },
    )?;

    // Start listening in another thread
    thread::spawn(move || {
        server.listen("/tmp/con-examples.sock").unwrap();
    });

    loop {
        let mut ctx = ctx.lock().unwrap();
        // Emit messages to all connected clients
        con::Server::broadcast(
            &server_state,
            "msg-from-server",
            Some(ctx.to_string().into_bytes()),
        )?;

        // Send message to 'client-a'
        con::Server::send(
            &server_state,
            "client-a",
            "some-msg",
            Some(Vec::from(format!(
                "This is special message for client-A: {}",
                ctx
            ))),
        )?;

        if *ctx == 10 {
            println!(" → Try to disconnect client-a");
            con::Server::disconnect(&server_state, "client-a").unwrap_or(());
        }

        *ctx += 1;
        drop(ctx);
        thread::sleep(time::Duration::from_millis(500));
    }
}
