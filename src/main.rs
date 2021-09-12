use std::collections::HashMap;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use std::ops::Div;
use serde_json::Value;

#[tokio::main]
async fn main() {
    monitor();
}

async fn monitor() {
    loop {
        let content = tweet_published().await;

        thread::spawn(move || {
            println!("tweet published:{}", content);
            let (tx, rx) = mpsc::channel();
            let base_price: f64 = get_btc_price();

            thread::spawn(move || {
                for i in 0..20 {
                    thread::sleep(Duration::from_secs(30));
                    let cur_price = get_btc_price();
                    let increase_range = (cur_price - base_price).div(base_price);
                    println!("cur_price:{}, increase_range:{:.3}%", cur_price, increase_range * 100.0);
                }
                tx.send("done").unwrap()
            });
            let received = rx.recv().unwrap();
            println!("Got: {}", received);
        });
    }
}

fn get_btc_price() -> f64 {
    let mut content;
    async {
        content = reqwest::get("https://api.huobi.pro/market/detail/merged?symbol=btcusdt").await.unwrap().text().await;
    }
    let mut values = serde_json::from_str(&(content.unwrap())).unwrap();
    println!("price:{}", values["tick"]["close"]);

    values["tick"]["close"].as_f64().unwrap()
}

async fn tweet_published() -> String {
    thread::sleep(Duration::from_secs(30));
    "asd".to_string()
}



