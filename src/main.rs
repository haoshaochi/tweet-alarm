use std::sync::{mpsc, Mutex, Arc};
use std::thread;
use std::time::Duration;
use std::ops::Div;
use serde_json::Value;
use std::sync::mpsc::Sender;
use std::future::Future;
use futures::executor::block_on;

#[tokio::main]
async fn main() {
    let mut db = Db {
        suc_cnt: 0,
        suc_rate: 0.0,
        last_time: 0,
        tweets: Vec::new(),
    };

    let suc_rate = db.suc_rate;
    let mtx = Arc::new(Mutex::new(db));
    loop {
        let tweet = tweet_published();
        println!("tweet published:{}, estimated success rate:{}", tweet.content, suc_rate);
        let lock = Arc::clone(&mtx);

        // thread::spawn(move||{
            let fut = monitor_and_alarm(lock, tweet);
            block_on(fut);
        // });
    }
}

async fn monitor_and_alarm(lock:Arc<Mutex<Db>>, tweet:Tweet){

    let range = output().await;
    let mut db = lock.lock().unwrap();
    db.add(tweet,range.ge(&0.0));
}

async fn output() ->f64{
    println!("try to get base data...");
    let btc_tick = get_btc_price().await.unwrap();
    println!("base_price:{},base_time:{}", btc_tick.price, btc_tick.ts);
    track_price(btc_tick.price, 2).await

}

async fn track_price(base_price: f64, times: i32) -> f64 {
    let mut range = 0.0;
    for i in 0..times {
        println!("track btc price, waiting for 30s...");
        thread::sleep(Duration::from_secs(30));
        let btc_tick = get_btc_price().await;
        if let Err(str) = btc_tick {
            println!("{}", str);
            continue;
        }
        let btc_tick = btc_tick.unwrap();
        range = (btc_tick.price - base_price).div(base_price);
        println!("price:{},time:{}, range:{:.5}%", btc_tick.price, btc_tick.ts, range * 100.0);
    }
    range
}

async fn get_btc_price() -> Result<BtcTick, String> {
    let resp = reqwest::get("https://api.huobi.pro/market/detail/merged?symbol=btcusdt").await;
    if let Ok(rs) = resp {
        let content = rs.text().await.unwrap();
        let values: Value = serde_json::from_str::<Value>(content.as_str()).unwrap();

        return Ok(BtcTick {
            ts: values["ts"].as_i64().unwrap(),
            price: values["tick"]["close"].as_f64().unwrap(),
        });
    }
    Err("访问btc url出错, 需要翻墙哦".to_string())
}

fn tweet_published() -> Tweet {
    println!("monitor tweets....");
    thread::sleep(Duration::from_secs(5));
    Tweet {
        time: 10,
        content: "asd".to_string(),
    }
}



pub struct BtcTick {
    ts: i64,
    price: f64,
}

pub struct Tweet {
    time: i64,
    content: String,
}

pub struct Db {
    pub last_time: i64,
    pub suc_rate: f64,
    pub suc_cnt: i32,
    pub tweets: Vec<Tweet>,
}

impl Db {
    pub fn add(&mut self, t: Tweet, result:bool) {
        self.last_time = t.time;
        self.tweets.push(t);

        if true == result {
            self.suc_cnt = self.suc_cnt + 1;
        }
        self.suc_rate = self.suc_cnt as f64 / self.tweets.len() as f64;
    }

}




