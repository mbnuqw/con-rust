extern crate rand;
extern crate con;

use std::time;
use std::thread;
use std::sync::{mpsc, Arc, Mutex};
use std::env;
use rand::prelude::*;
use con::Error;
use con::Client;

fn main() -> Result<(), Error> {
    let args: Vec<String> = env::args().collect();
    let name = match &args.get(1) {
        Some(n) => n,
        None => "default-name",
    };

    // Connect to server
    let ctx = Arc::new(Mutex::new(0u64));
    // let mut client = Client::connect("/tmp/con-examples.sock", ctx.clone(), Some(&name))?;
    let mut client = Client::connect("127.0.0.1:1234", ctx.clone(), Some(&name))?;

    loop {
        // Sleep for some random time...
        let mut rng = thread_rng();
        let rd: u8 = rng.gen();
        thread::sleep(time::Duration::from_millis((rd as u64) << 1));

        // Make request
        let (ans_rx, ans_tx) = mpsc::channel();
        client.req("repeat", Some(Vec::from("this")), ans_rx);
        if let Some(_) = ans_tx.recv().unwrap() {
            // println!(" → Repeat result: {:?}", String::from_utf8_lossy(&ans));
        }
    }
}