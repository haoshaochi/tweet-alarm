use chrono::{NaiveDateTime, DateTime, Local, TimeZone};
use std::process::Command;
use std::ops::{Div, Sub};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use serde_json::Value;
use tokio::runtime::Runtime;
use time::Duration as DurationX;


///
/// twint - the tool to extract tweets from twitter.com
/// 安装方法:
/// pip3 install --user --upgrade -e git+https://github.com/twintproject/twint.git@origin/master#egg=twint
///
const WINDOWS_TWINT_PATH: &str = "E:\\develop\\python\\Scripts\\twint";
const LINUX_TWINT_PATH: &str = "./twint";
// twint command string
const CMD_STR: &str = "{TWINT} -u {USER} --since {DATE} -o {FILE_PATH} --json --limit 1";

// 跟踪的twitter用户
const TWEET_USER_NAME: &str = "RunzeHao";
//ElonMusk
// tweet信息存储路径
const TWEET_FILE_PATH: &str = "./file.txt";

//btc行情获取地址，需要翻墙
const BTC_TICK_URL: &str = "https://api.huobi.pro/market/detail/merged?symbol=btcusdt";

// tweet后, btc价格追踪次数
const TRACK_PRICE_CNT: i32 = 10;
// btc价格追踪的时间间隔
const TRACK_PRICE_INTERVAL: i32 = 30;


///
/// btc 行情信息
///
pub struct BtcTick {
    ts: i64,
    price: f64,
}

///
/// Tweet 内容
///
#[derive(Debug)]
pub struct Tweet {
    // 时间戳:秒
    time: i64,
    // 内容
    content: String,
}

///
/// tweets 数据库
///
#[derive(Debug)]
pub struct TweetDb {
    // 上次tweet时间
    pub last_time: i64,
    // tweet 发布后btc 上涨概率
    pub suc_rate: f64,
    // tweet 发布后btc 上涨次数
    pub suc_cnt: i32,
    pub tweets: Vec<Tweet>,
}

impl TweetDb {
    ///
    /// 数据库中增加tweet
    /// 并计算上涨概率
    ///
    pub fn add(&mut self, t: Tweet, result: bool) {
        self.last_time = t.time;
        self.tweets.push(t);

        if true == result {
            self.suc_cnt = self.suc_cnt + 1;
        }
        self.suc_rate = self.suc_cnt as f64 / self.tweets.len() as f64;
    }
}


fn main() {
    // 初始化 db
    let db = TweetDb {
        suc_cnt: 0,
        suc_rate: 0.0,
        last_time: 0,
        tweets: Vec::new(),
    };

    //tweet的初始时间为2分钟前，追踪btc price才有意义
    let mut last_time = two_minutes_before();
    let mtx = Arc::new(Mutex::new(db));
    let mut result_list = Vec::new();

    loop {
        //扫描是否有新的tweet发布, 如果返回成功则开启线程去处理tweet并更新db
        match monitor_tweet(&last_time) {
            Ok(tweet) => {
                last_time = tweet.time;
                let lock = Arc::clone(&mtx);
                // alarm
                println!("[tid:{}]tweet published, estimated success rate:{:.3}%", tweet.time, lock.lock().unwrap().suc_rate * 100.0);

                //新建线程去处理每个新的tweet
                let ans = thread::spawn(move || {
                    output(lock, tweet);
                });
                result_list.push(ans);
            }
            Err(str) => println!("monitor tweet result: {}", str),
        }

        println!("30s后再次扫描twitter....");
        thread::sleep(Duration::from_secs(30));
    }
}

///
/// 取2分钟前的时间
///
fn two_minutes_before() -> i64 {
    Local::now().sub(DurationX::seconds(2)).timestamp()
}

///
/// 输出btc初始价格 并持续追踪价格变化
/// 并将新的tweet信息和成功率率更新到db
///
fn output(lock: Arc<Mutex<TweetDb>>, tweet: Tweet) {
    let range = Runtime::new()
        .expect("Failed to create runtime")
        .block_on(output_btc_tick(&(tweet.time)));

    let mut db = lock.lock().unwrap();
    db.add(tweet, range.ge(&0.0));
    println!("当前tweets db信息:{:#?}", db);
}

///
/// 输出btc初始价格
/// 并持续追踪价格变化
///
async fn output_btc_tick(tid: &i64) -> f64 {
    println!("[tid:{}]try to get base data...", tid);
    let btc_tick = get_btc_price().await.unwrap();
    println!("[tid:{}]base_price:{},base_time:{}", tid, btc_tick.price, btc_tick.ts);
    track_price(tid, btc_tick.price).await
}

///
/// 持续跟踪 btc 价格并输出
///
async fn track_price(tid: &i64, base_price: f64) -> f64 {
    let mut range = 0.0;
    for i in 0..TRACK_PRICE_CNT {
        println!("[tid:{}]track btc price({}), waiting for {}s...", tid,i, TRACK_PRICE_INTERVAL);
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
    let resp = reqwest::get(BTC_TICK_URL).await;
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

///
/// 检查是否有tweet发布
/// 有则返回Tweet结构
/// 没有或访问异常返回Err
///
fn monitor_tweet(last_time: &i64) -> Result<Tweet, String> {
    //1. 生成 抓取tweets 的命令
    let cmd_str = gen_cmd(last_time);

    //2. 执行命令
    let output = if cfg!(target_os = "windows") {
        Command::new("cmd").arg("/c").arg(cmd_str).output().expect("cmd exec error!")
    } else {
        Command::new("sh").arg("-c").arg(cmd_str).output().expect("sh exec error!")
    };

    let output_str = String::from_utf8_lossy(&output.stdout);
    let output_err = String::from_utf8_lossy(&output.stderr);
    println!("获取tweets结果为：{}", output_str);


    //3. 处理返回结果，未抓取到tweets直接返回
    if output_str.is_empty() {
        println!("扫描twitter失败，确认是否已翻墙，err msg:{}", output_err);
        return Err(output_err.to_string());
    } else if output_str.starts_with("[!]") {
        return Err("no tweet published".to_string());
    }

    //4. 处理抓取的tweets
    let tweets:Vec<_> = output_str.split("\n").collect();
    let first = tweets.get(0).unwrap().to_string();
    println!("当前最新tweet:{}", first);
    let first_vec:Vec<_> = first.split(" ").collect();

    //4.1 分析tweet内容
    let str_date = first_vec.get(1).unwrap().to_string() + " " + first_vec.get(2).unwrap() + first_vec.get(3).unwrap();

    let mut tweet_content = String::new();
    for i in 5..first_vec.len() {
        tweet_content += first_vec.get(i).unwrap();
    }
    println!("tweet_date:{},content:{}",str_date,tweet_content);

    /*let file = File::open(String::from(TWEET_FILE_PATH));
    if file.is_err() {
        return Err(file.err().unwrap().to_string());
    }
    let mut fin = std::io::BufReader::new(file.unwrap());
    let mut line = String::new();
    fin.read_line(&mut line).unwrap();
    println!("当前最新tweet:{}", line);
    let values: Value = serde_json::from_str::<Value>(&line.as_str()).unwrap();
    let str_date = values["date"].as_str().unwrap().to_string() + " " + values["time"].as_str().unwrap() + values["timezone"].as_str().unwrap();*/

    let fmt = "%Y-%m-%d %H:%M:%S %z";
    let result = DateTime::parse_from_str(str_date.as_str(), fmt);

    if result.is_err() {
        println!("{}", result.err().unwrap());
        return Err("datetime parse error".to_string());
    }
    let secs = result.unwrap().timestamp();

    //4.2 检查是否已经处理过
    if secs.le(last_time) {
        return Err("not new tweet".to_string());
    }

    Ok(Tweet {
        time: secs,
        content: tweet_content,
    })
}

///
/// 生成 twint 命令
///
fn gen_cmd(last_time: &i64) -> String {
    let twint_str = if cfg!(target_os = "windows") {
        WINDOWS_TWINT_PATH
    } else {
        LINUX_TWINT_PATH
    };
    let fmt = "%Y-%m-%d";
    let naive = NaiveDateTime::from_timestamp(*last_time, 0);
    let start_date = Local.from_utc_datetime(&naive).date().format(fmt).to_string();

    let cmd_str = CMD_STR
        .replace("{TWINT}", twint_str)
        .replace("{USER}", TWEET_USER_NAME)
        .replace("{DATE}", start_date.as_str())
        .replace("{FILE_PATH}", TWEET_FILE_PATH);

    println!("执行命令:{}", cmd_str);
    cmd_str
}





