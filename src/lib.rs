extern crate reqwest;
use reqwest::{header, RedirectPolicy};
pub type Error = failure::Error;
pub type Result<T> = std::result::Result<T, Error>;

extern crate failure;
use failure::bail;
extern crate crypto;
use crypto::digest::Digest;
use crypto::sha1::Sha1;
use std::env;


const WANAPLAY_END_POINT: &str = "http://fr.wanaplay.com/";

pub fn wanaplay_route(route: &str) -> String {
    format!("{}{}", WANAPLAY_END_POINT, route)
}


pub struct WanaplayCredentials {
    pub login: String,
    pub password: WanaplayPassword,
}

pub struct WanaplayPassword {
    pub secret_password: String,
}

impl WanaplayPassword {
    pub fn crypted(&self) -> String {
        let mut hasher = Sha1::new();
        hasher.input_str(self.secret_password.as_str());
        hasher.result_str()
    }
}



pub fn authenticate(login: String, crypted_password: String) -> Result<reqwest::Client> {
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
        bail!("unable to login");
    }
    let session_cookie = authent_response.headers().get(header::SET_COOKIE).unwrap();
    let mut headers = header::HeaderMap::new();
    headers.insert(header::COOKIE, session_cookie.clone());
    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .unwrap();
    // useless request but mandatory :/
    client
        .post(wanaplay_route("reservation/planning2").as_str())
        .form(&[("date", "2018-12-24")])
        .send()
        .unwrap();
    Ok(client)
}

pub fn get_logged_client() -> Result<reqwest::Client> {
    let credentials = WanaplayCredentials {
        login: env::var("wanaplay_login").unwrap(),
        password: WanaplayPassword {
            secret_password: env::var("wanaplay_password").unwrap(),
        },
    };
    authenticate(credentials.login, credentials.password.crypted())
}