use std::sync::{Mutex, Arc};
use std::thread;
use std::time::Duration;
use std::ops::Div;
use serde_json::Value;
use tokio::runtime::Runtime;
use chrono::Local;

// 跟踪价格 时间间隔
const TRACK_PRICE_INTERVAL: i32 = 10;
// 跟踪次数
const TRACK_PRICE_CNT: i32 = 2;
// TWITTER USER ID
const TWITTER_ID: i64 = 123;


fn main() {
    let db = Db {
        suc_cnt: 0,
        suc_rate: 0.0,
        last_time: 0,
        tweets: Vec::new(),
    };

    let mtx = Arc::new(Mutex::new(db));

    let mut result_list = Vec::new();

    loop {
        //扫描是否有新的tweet发布

        let tweet = tweet_published();
        let lock = Arc::clone(&mtx);
        println!("[tid:{}]tweet published, estimated success rate:{:.3}%", tweet.time, lock.lock().unwrap().suc_rate * 100.0);


        //新建线程去处理每个新的tweet
        let ans = thread::spawn(move || {
            monitor_and_alarm(lock, tweet);
        });
        result_list.push(ans);
    }
}

fn monitor_and_alarm(lock: Arc<Mutex<Db>>, tweet: Tweet) {
    let range = Runtime::new()
        .expect("Failed to create runtime")
        .block_on(output(&(tweet.time)));

    let mut db = lock.lock().unwrap();
    db.add(tweet, range.ge(&0.0));
    println!("{:#?}", db);
}

async fn output(tid: &i64) -> f64 {
    println!("[tid:{}]try to get base data...", tid);
    let btc_tick = get_btc_price().await.unwrap();
    println!("[tid:{}]base_price:{},base_time:{}", tid, btc_tick.price, btc_tick.ts);
    track_price(tid, btc_tick.price, TRACK_PRICE_CNT).await
}

async fn track_price(tid: &i64, base_price: f64, times: i32) -> f64 {
    let mut range = 0.0;
    for _i in 0..times {
        println!("[tid:{}]track btc price, waiting for {}s...", tid, TRACK_PRICE_INTERVAL);
        thread::sleep(Duration::from_secs(TRACK_PRICE_INTERVAL as u64));
        let btc_tick = get_btc_price().await;
        if let Err(str) = btc_tick {
            println!("{}", str);
            continue;
        }
        let btc_tick = btc_tick.unwrap();
        range = (btc_tick.price - base_price).div(base_price);
        println!("[tid:{}]price:{},time:{}, range:{:.5}%", tid, btc_tick.price, btc_tick.ts, range * 100.0);
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
    thread::sleep(Duration::from_secs(20));
    Tweet {
        time: Local::now().timestamp_millis(),
        content: "asd".to_string(),
    }
}


pub struct BtcTick {
    ts: i64,
    price: f64,
}

#[derive(Debug)]
pub struct Tweet {
    time: i64,
    content: String,
}

#[derive(Debug)]
pub struct Db {
    pub last_time: i64,
    pub suc_rate: f64,
    pub suc_cnt: i32,
    pub tweets: Vec<Tweet>,
}

impl Db {
    pub fn add(&mut self, t: Tweet, result: bool) {
        self.last_time = t.time;
        self.tweets.push(t);

        if true == result {
            self.suc_cnt = self.suc_cnt + 1;
        }
        self.suc_rate = self.suc_cnt as f64 / self.tweets.len() as f64;
    }
}




