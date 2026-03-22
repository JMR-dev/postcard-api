use std::time::Duration;
use std::thread;

fn main() {
    println!("Backend stub started...");
    loop {
        thread::sleep(Duration::from_secs(60));
    }
}
