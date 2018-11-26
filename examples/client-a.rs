extern crate con;

use std::time;
use std::thread;
use std::sync::{mpsc, Arc, Mutex};
use con::Error;
use con::Client;
use con::MsgName;

fn main() -> Result<(), Error> {
    println!(" → Client A");

    let ctx = Arc::new(Mutex::new(0u64));
    let mut client = Client::connect("/tmp/con-examples.sock", ctx.clone(), Some("client-a"))?;

    // Send message
    println!(" → Send 'msg-A' with body 'Just body'");
    client.send("msg-A", Some(Vec::from("Just body...")));

    // Request
    let (ans_rx, ans_tx) = mpsc::channel();
    println!(" → Request 'repeat' with body 'this'");
    client.req("repeat", Some(Vec::from("this")), ans_rx);
    if let Some(ans) = ans_tx.recv().unwrap() {
        println!(" → Repeat result: {:?}", String::from_utf8_lossy(&ans));
    }

    // Subscribe
    client.on(MsgName::Any, |msg, _state, _ctx| {
        let body = match msg.body {
            Some(b) => String::from_utf8_lossy(&b).to_string(),
            None => "<Nothing>".to_string(),
        };
        println!(" → Got msg form server: {:?}", body);
    });

    loop {
        thread::sleep(time::Duration::from_millis(1000));
    }
}