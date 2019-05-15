extern crate chrono;
extern crate env_logger;
use chrono::prelude::*;
use std::env;
use std::thread;
use std::time;
extern crate failure;
extern crate select;
pub type Error = failure::Error;
pub type Result<T> = std::result::Result<T, Error>;
use itertools::join;
use wanaplay_booker::*;

fn main() {
    env_logger::init();
    if let Err(err) = run() {
        for cause in err.iter_chain() {
            eprintln!("{}", cause);
        }
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let bot_token = env::var("bot_token")?;
    let chat_id = env::var("chat_id")?;
    let client = reqwest::Client::new();
    loop {
        let now: DateTime<Local> = match env::var("fake_date") {
            Ok(fake_date) => fake_date.parse::<DateTime<Local>>().unwrap(),
            Err(_) => Local::now(),
        };
        println!("loop {:?}", now);
        if now.hour() == 9 {
            let bookings = get_bookings();
            let today_bookings = bookings
                .into_iter()
                .filter(|booking| booking.date == now.naive_local().date())
                .collect::<Vec<_>>();
            if !today_bookings.is_empty() {
                let bookings_number = today_bookings.len();
                println!("found {} bookings for today", bookings_number);
                let details = join(
                    today_bookings
                        .iter()
                        .map(|booking| booking.court_time.clone()),
                    " and ",
                );
                let params = [
                    ("chat_id", &chat_id),
                    (
                        "text",
                        &format!(
                            "{} bookings scheduled for today at {}",
                            bookings_number, details
                        )
                        .to_string(),
                    ),
                ];
                client
                    .post(format!("https://api.telegram.org/bot{}/sendMessage", bot_token).as_str())
                    .form(&params)
                    .send()?;
                println!("sleep for 1d");
                thread::sleep(time::Duration::from_secs(24 * 60 * 60));
            }
        } else {
            println!("sleep for 1h");
            thread::sleep(time::Duration::from_secs(60 * 60));
        }
    }
}
