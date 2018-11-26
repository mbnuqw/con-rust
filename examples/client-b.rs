extern crate con;

use std::time;
use std::thread;
use std::sync::{Arc, Mutex};
use con::Error;
use con::Client;
use con::MsgName;

fn main() -> Result<(), Error> {
    println!(" → Client B");

    let ctx = Arc::new(Mutex::new(0u64));
    let mut client = Client::connect("/tmp/con-examples.sock", ctx.clone(), Some("client-b"))?;

    // Send messages
    client.send("msg-A", None);
    client.send("another msg", Some(Vec::from("with body")));

    // Subscribe
	client.on(MsgName::Is("msg-from-server"), |msg, _state, _ctx| {
        let body = match msg.body {
            Some(b) => String::from_utf8_lossy(&b).to_string(),
            None => "<Nothing>".to_string(),
        };
        println!(" → Got msg from server {:?}", body);
	});

    loop {
        thread::sleep(time::Duration::from_millis(1000));
    }
}