extern crate con;

use std::time;
use std::sync::{mpsc, Arc, Mutex};
use con::Error;
use con::Client;
use con::utils;

fn main() -> Result<(), Error> {
    println!(" → Client A");

    let ctx = Arc::new(Mutex::new(0u64));
    let mut client = Client::connect("/tmp/con-examples.sock", ctx.clone(), Some("client-a"))?;

    // Make 1000 requests
    let mut answers = Vec::with_capacity(1000);
    let start_ts = time::Instant::now();
    for i in 0..1000 {
        let (ans_rx, ans_tx) = mpsc::channel();
        answers.push(ans_tx);
        let s = format!("{}.{}", i, utils::uid());
        client.req("repeat", Some(Vec::from(s)), ans_rx);
    }
    println!(" → {:?}", start_ts.elapsed());

    for ans in answers {
        if let Some(_ans) = ans.recv().unwrap() {
            // println!(" → Repeat result: {:?}", String::from_utf8_lossy(&ans));
        }
    }

    Ok(())
}