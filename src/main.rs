extern crate reqwest;
use reqwest::{header, RedirectPolicy};
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT, CONTENT_TYPE};
extern crate crypto;
extern crate env_logger;
use crypto::digest::Digest;
use crypto::sha1::Sha1;
extern crate structopt;
use structopt::StructOpt;
extern crate chrono;
use chrono::prelude::*;
use chrono::Duration;
use chrono::NaiveTime;
use chrono::Weekday;
use std::env;
use std::thread;
use std::time;
extern crate scraper;
use scraper::Html;
use scraper::Selector;
extern crate select;
use select::document::Document;
use select::predicate::{Class, Name, Predicate};

const WANAPLAY_END_POINT: &str = "http://fr.wanaplay.com/";

struct WanaplayPassword {
    secret_password: String,
}

impl WanaplayPassword {
    pub fn crypted(&self) -> String {
        let mut hasher = Sha1::new();
        hasher.input_str(self.secret_password.as_str());
        hasher.result_str()
    }
}

struct WanaplayCredentials {
    login: String,
    password: WanaplayPassword,
}

struct Parameters {
    weekday: Weekday,
    court_time: NaiveTime,
    wanaplay_credentials: WanaplayCredentials,
}

#[derive(Debug, StructOpt)]
#[structopt(name = "wanaplay-booker", about = " Book a court when available")]
struct Opt {
    /// week day to book
    #[structopt(short = "w", long = "weekday")]
    weekday: String,
    /// court time
    #[structopt(short = "c", long = "court_time")]
    court_time: NaiveTime,
}

fn validate_args(opt: &mut Opt) -> Parameters {
    let mut valid_times = vec![];
    let mut start_time = NaiveTime::from_hms(9, 0, 0);
    let end_time = NaiveTime::from_hms(23, 0, 0);
    while start_time <= end_time {
        valid_times.push(start_time.clone());
        start_time = match start_time.overflowing_add_signed(Duration::minutes(40)) {
            (val, _) => val,
        };
    }
    if !valid_times.contains(&opt.court_time) {
        panic!(
            "{} is not a valid court time, should be one of {:?}",
            opt.court_time, valid_times
        );
    }
    let weekday = opt
        .weekday
        .parse::<Weekday>()
        .unwrap_or_else(|_| panic!("{} is not a valid week day", opt.weekday));
    Parameters {
        weekday,
        court_time: opt.court_time,
        wanaplay_credentials: WanaplayCredentials {
            login: env::var("wanaplay_login").unwrap(),
            password: WanaplayPassword {
                secret_password: env::var("wanaplay_password").unwrap(),
            },
        },
    }
}

fn wanaplay_route(route: &str) -> String {
    format!("{}{}", WANAPLAY_END_POINT, route)
}

fn is_openned(target_date: NaiveDate) -> bool {
    let forbidden = "Vous ne pouvez pas voir le planning";
    let client = reqwest::Client::new();
    println!("watch_openning {:?}", target_date);
    let mut response = client
        .post(wanaplay_route("reservation/planning2").as_str())
        .form(&[("date", target_date.format("%Y-%m-%d").to_string())])
        .send()
        .unwrap();
    !response.text().unwrap().contains(forbidden)
}

fn book(target_date: NaiveDate, court_time: NaiveTime, login: String, crypted_password: String) {
    println!("book {:?} at {:?}", target_date, court_time);
    let authent_client = reqwest::Client::builder()
        .redirect(RedirectPolicy::none())
        .build()
        .unwrap();
    let authent_response = authent_client
        .post(wanaplay_route("auth/doLogin").as_str())
        .form(&[("login", login), ("sha1mdp", crypted_password)])
        .send()
        .unwrap();
    let location = authent_response.headers().get(header::LOCATION);
    if location.is_none()
        || location.unwrap().to_str().unwrap() != wanaplay_route("auth/infos").as_str()
    {
        panic!("unable to login");
    }
    let session_cookie = authent_response.headers().get(header::SET_COOKIE).unwrap();
    let mut headers = header::HeaderMap::new();
    headers.insert(header::COOKIE, session_cookie.clone());
    println!("{:?}", headers);
    println!("{:?}", target_date.format("%Y-%m-%d").to_string());
    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .unwrap();
    // useless request
    let mut response = client
        .post(wanaplay_route("reservation/planning2").as_str())
        .form(&[("date", "2018-12-24")])
        .send()
        .unwrap();

    let mut response = client
        .post(wanaplay_route("reservation/planning2").as_str())
        .form(&[("date", target_date.format("%Y-%m-%d").to_string())])
        .send()
        .unwrap();

    let document = Document::from_read(response).unwrap();
    println!("{:?}", document.find(Class("timeSlotTime")).count());
    println!("{:?}", document.find(Class("creneauLibre")).count());
    println!("{:?}", court_time.format("%H:%M"));
    let ids = document.find(Class("creneauLibre"))
        .filter(|node| node.children().next().unwrap().children().next().unwrap().text() == court_time.format("%H:%M").to_string())
        .filter(|node| node.attr("class").unwrap() == "creneauLibre")
        .map(|node| node.attr("onclick").unwrap())
        .map(|link| link.split("idTspl=").collect::<Vec<_>>()[1].replace("\"", ""))
        .collect::<Vec<_>>();
    println!("{:?}", ids);

}

fn main() {
    let mut opt = Opt::from_args();
    println!("{:?}", opt);
    let parameters = validate_args(&mut opt);
    book(
        NaiveDate::from_ymd(2019, 1, 2),
        parameters.court_time,
        parameters.wanaplay_credentials.login.clone(),
        parameters.wanaplay_credentials.password.crypted(),
    );
    loop {
        let now: DateTime<Local> = Local::now();
        println!("loop {:?}", now);
        if now.weekday() == parameters.weekday.pred() {
            let target_date = now + Duration::days(15);
            let target_date =
                NaiveDate::from_ymd(target_date.year(), target_date.month(), target_date.day());
            println!("target_date = {:?}", target_date);
            if now.hour() == 23 {
                if now.minute() >= 55 {
                    while !is_openned(target_date) {
                        thread::sleep(time::Duration::from_secs(2));
                    }
                    book(
                        target_date,
                        parameters.court_time,
                        parameters.wanaplay_credentials.login.clone(),
                        parameters.wanaplay_credentials.password.crypted(),
                    );
                } else {
                    println!("sleep for 1 min");
                    thread::sleep(time::Duration::from_secs(60));
                }
            } else {
                println!("sleep for 55 min");
                thread::sleep(time::Duration::from_secs(55 * 60));
            }
        } else {
            println!("sleep for 23h");
            thread::sleep(time::Duration::from_secs(23 * 60 * 60));
        }
    }
}
