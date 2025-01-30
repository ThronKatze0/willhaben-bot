use ntfy::prelude::*;
use std::collections::HashMap;
use std::time::Duration;
use std::{fs, os::unix::raw};

use cookie::Expiration;
use cookie::time::OffsetDateTime;
use fantoccini::Client;
use fantoccini::{ClientBuilder, Locator, cookies::Cookie};
use serde::{Deserialize, Serialize};
use tokio::time;

const DOMAIN: &str = "www.willhaben.at";
const URL: &str = "https://willhaben.at/iad/kaufen-und-verkaufen/marktplatz?isNavigation=true&srcType=vertical-search-box&keyword=KIZ%20konzert%20wien";

#[derive(Debug, Serialize, Deserialize)]
struct RawCookie {
    name: String,
    value: String,
    domain: String,
    path: Option<String>,
    secure: Option<bool>,
    httpOnly: Option<bool>,
    expiry: Option<i64>,
    session: Option<bool>,
}

#[derive(Debug)]
struct WillhabenAd {
    title: String,
    location: String,
    price: f32,
}

impl WillhabenAd {
    fn new(title: String, location: String, price: f32) -> WillhabenAd {
        WillhabenAd {
            title,
            location,
            price,
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), fantoccini::error::CmdError> {
    let c = ClientBuilder::native()
        .connect("http://localhost:4444")
        .await
        .expect("failed to connect to WebDriver");

    c.goto(URL).await?;
    time::sleep(Duration::from_secs(2)).await;
    login(&c, "cookies.json").await?;

    let mut willhaben_ads = get_ads(&c).await?;
    loop {
        println!("Scanning...");
        c.goto(URL).await?;
        time::sleep(Duration::from_secs(6)).await;
        let curr_ads = get_ads(&c).await?;
        for ad in curr_ads.iter().map(|(_, ad)| ad) {
            if !willhaben_ads.contains_key(&ad.title) {
                println!(
                    "New Ad found!!! Title: {}, Price: {}, Messaging...",
                    &ad.title, &ad.price
                );
                let name = message_ad(&c, &ad.location).await?;
                println!("Messaged {}", name);
            }
        }
        willhaben_ads = curr_ads;
        time::sleep(Duration::from_secs(12)).await;
    }
}

async fn login(c: &Client, cookie_file: &str) -> Result<(), fantoccini::error::CmdError> {
    c.delete_all_cookies().await?;

    let cookies_json = fs::read_to_string(cookie_file)?;
    let raw_cookies: Vec<RawCookie> = serde_json::from_str(&cookies_json)?;

    for raw_cookie in raw_cookies {
        let mut cookie = Cookie::new(raw_cookie.name, raw_cookie.value);
        cookie.set_domain(raw_cookie.domain);
        cookie.set_path(raw_cookie.path.unwrap());
        cookie.set_secure(raw_cookie.secure.unwrap());
        cookie.set_http_only(raw_cookie.httpOnly.unwrap());
        match raw_cookie.session {
            Some(_) => cookie.set_expires(Expiration::Session),
            None => cookie.set_expires(Expiration::DateTime(
                OffsetDateTime::from_unix_timestamp(raw_cookie.expiry.unwrap()).unwrap(),
            )),
        }
        c.add_cookie(cookie).await?;
    }
    c.goto(URL).await?;
    time::sleep(Duration::from_secs(2)).await;
    Ok(())
}

async fn get_ads(c: &Client) -> Result<HashMap<String, WillhabenAd>, fantoccini::error::CmdError> {
    let raw_ad_list = c
        .find(Locator::Id("skip-to-resultlist"))
        .await?
        .find_all(Locator::Css(".Box-sc-wfmb7k-0"))
        .await?;

    let mut ads = HashMap::new();
    for elem in raw_ad_list {
        let title = match elem.find_all(Locator::Css("h3")).await?.get(0) {
            Some(title) => title.text().await?,
            None => continue,
        };
        let location = match elem.find_all(Locator::Css("a")).await?.get(0) {
            Some(location) => location.attr("href").await?.unwrap(),
            None => continue,
        };
        let raw_price: String = match elem.find_all(Locator::Css(".tElSx")).await?.get(0) {
            Some(raw_price) => raw_price.text().await?,
            None => continue,
        };
        let price: f32 = raw_price
            .split(" ")
            .nth(1)
            .unwrap()
            .trim()
            .replace(",", ".")
            .parse()
            .unwrap();
        let ad = WillhabenAd::new(title.clone(), location, price);
        ads.entry(title).or_insert(ad);
    }

    return Ok(ads);
}

async fn message_ad(c: &Client, location: &str) -> Result<String, fantoccini::error::CmdError> {
    c.goto(&format!("https://{}{}", DOMAIN, location)).await?;
    time::sleep(Duration::from_secs(2)).await;
    let name = c
        .find(Locator::Css(
            ".jYVNrL > div:nth-child(1) > div:nth-child(2) > div:nth-child(1) > span:nth-child(1)",
        ))
        .await?
        .text()
        .await?;
    time::sleep(Duration::from_secs(7)).await;
    c.find(Locator::Id("mailContent"))
        .await?
        .send_keys(&format!(
            "Hallo {}, ich möchte die Tickets bitte gleich reservieren und kaufen",
            name
        ))
        .await?;
    time::sleep(Duration::from_secs(2)).await;
    c.find(Locator::Css(".GSQoz")).await?.click().await?;
    time::sleep(Duration::from_secs(2)).await;
    return Ok(name);
}
