/*
    VCP Restart - restart nc servers based upon URL reachbaility
    Copyright (C) 2020  Aron Heinecke

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU Affero General Public License as
    published by the Free Software Foundation, either version 3 of the
    License, or (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU Affero General Public License for more details.

    You should have received a copy of the GNU Affero General Public License
    along with this program.  If not, see <https://www.gnu.org/licenses/>.
*/
use std::{collections::HashMap, fs, thread::sleep, time::{Duration, Instant}};
use env_logger::Target;
use log::*;
use reqwest::{blocking::*, header};
use stable_eyre::eyre::Result;
use toml::from_str;
use serde::Deserialize;
use regex::Regex;
fn main() -> Result<()>{
    env_logger::builder().filter(Some("vcp_monitoring"),LevelFilter::max()).target(Target::Stdout).try_init()?;
    info!("Hello, world!");
    
    let config = fs::read_to_string(".credentials.toml")?;
    let config: Config = from_str(&config)?;
    let mut headers = header::HeaderMap::new();
    headers.insert(header::PRAGMA, header::HeaderValue::from_static("no-cache"));
    headers.insert(header::ACCEPT_LANGUAGE, header::HeaderValue::from_static("de,en-US;q=0.7,en;q=0.3"));
    headers.insert(header::USER_AGENT, header::HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:83.0) Gecko/20100101 Firefox/83.0"));
    headers.insert(header::UPGRADE_INSECURE_REQUESTS, header::HeaderValue::from_static("1"));
    headers.insert(header::REFERER, header::HeaderValue::from_static("https://www.servercontrolpanel.de"));
    headers.insert(header::ACCEPT, header::HeaderValue::from_static("text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8"));
    headers.insert(header::ACCEPT_ENCODING, header::HeaderValue::from_static("gzip, deflate, br"));
    let client = Client::builder().cookie_store(true).default_headers(headers).build()?;
    let duration = Duration::from_secs(config.cooldown_s);
    // restart_server(&client,&config)?;
    info!("Testing login..");
    restart_server(&client,&config,true)?;
    info!("Passed login test");
    // return Ok(());
    loop {
        let restart = match client.get(config.url.as_str()).send().and_then(|v|v.error_for_status()) {
            Err(_e) => true,
            Ok(_v) => {
                false
            }
        };

        if restart {
            debug!("Restarting..");
            info!("Restarting..");
            if let Err(e ) = restart_server(&client,&config,false) {
                error!("Unable to restart: {}",e);
            }
            debug!("Cooldown after restart for {}s",duration.as_secs());
            sleep_secs(duration);
        } else {
            trace!("Server ok, sleeping..");
            sleep_secs(Duration::from_secs(10));
        }
    }
}

fn sleep_secs(dur: Duration) {
    let now = Instant::now();
    while now.elapsed() < dur {
        sleep(dur - now.elapsed());
    }
}

fn restart_server(client: &Client, config: &Config, simulate: bool) -> Result<()> {
    // get site_key form hidden field
    let request = client.get("https://www.servercontrolpanel.de/SCP/Home").send()?;
    debug!("{:?}",&request);
    let text = &request.text()?;
    debug!("{:?}",&text);
    let reg_sk = Regex::new(r#"input type="hidden".*name="site_key".*value="([A-Za-z0-9]+)""#).unwrap();
    let cap = reg_sk.captures(&text).unwrap();
    let site_key = &cap[1];
    debug!("site_key: {}",site_key);
  
    // login & get server ID from "server selection" screen 
    let mut params = HashMap::new();
    params.insert("site_key", site_key);
    params.insert("username", config.user.as_str());
    params.insert("password", config.password.as_str());
    let res = client.post("https://www.servercontrolpanel.de/SCP/Login")
        .form(&params).send()?;
    debug!("{:?}",&res);
    let text = res.text()?;
    debug!("debug login {}",&text);

    let reg_sid = Regex::new(&format!(r#"links\[["']{}["']\]\s+=\s+["'].*selectedVServerId=(\d+)["']"#,config.server_id)).unwrap();
    let cap = reg_sid.captures(&text).unwrap();
    let sid: i64 = cap[1].parse()?;

    let reg_sk = Regex::new(r#"site_key\s=\s["']([A-Za-z0-9]+)["'];"#).unwrap();
    let site_key = &reg_sk.captures(&text).unwrap()[1];
    let referer = format!("Referer: https://www.servercontrolpanel.de/SCP/Home?site_key={}",site_key);
    
    debug!("SID {} site_key: {}",sid,site_key);

    // select server 1
    let cmd = format!("https://www.servercontrolpanel.de/SCP/VServersKVM?selectedVServerId={}&site_key={}",sid,site_key);
    let res = client.get(&cmd)
        .header(header::ACCEPT, "text/html, */*; q=0.01")
        .header(header::REFERER, &referer).send()?;
    debug!("{:?}",&res);
    let text = res.text()?;
    debug!("debug select server 1 {}",&text);
    let site_key = &reg_sk.captures(&text).unwrap()[1];
    
    // select server 2
    let cmd = format!("https://www.servercontrolpanel.de/SCP/VServersKVM?selectedVServerId={}&page=vServerKVMGeneral&site_key={}",sid,site_key);
    let res = client.get(&cmd)
    .header(header::ACCEPT, "*/*")
    .header(header::REFERER, &referer).send()?;
    debug!("{:?}",&res);
    let text = res.text()?;
    debug!("debug select server 2 {}",&text);
    let site_key = &reg_sk.captures(&text).unwrap()[1];

    // navigate to control screen
    let cmd = format!("https://www.servercontrolpanel.de/SCP/VServersKVM?selectedVServerId={}&page=vServerKVMControl&site_key={}",sid,site_key);
    let res = client.get(&cmd)
    .header(header::ACCEPT, "text/html, */*; q=0.01")
    .header(header::REFERER, &referer).send()?;
    debug!("{:?}",&res);
    let text = res.text()?;
    debug!("debug control page {}",&text);
    let mut site_key = reg_sk.captures(&text).unwrap()[1].to_owned();

    // perform action
    // VServersKVM?selectedVServerId=123&page=vServerKVMControl&action=POWERCYCLE
    if simulate {
        info!("Simulating SCP..");
    } else {
        let cmd = format!("https://www.servercontrolpanel.de/SCP/VServersKVM?selectedVServerId={}&page=vServerKVMControl&action=POWERCYCLE&site_key={}",sid,site_key);
        let res = client.post(&cmd)
        .header(header::ACCEPT, "text/html, */*; q=0.01")
        .header(header::REFERER, &referer).send()?;
        debug!("{:?}",&res);
        let text = res.text()?;
        debug!("debug powercycle {}",&text);
        site_key = reg_sk.captures(&text).unwrap()[1].to_owned();
    }

    // logout
    // https://www.servercontrolpanel.de/SCP/Logout?site_key=ASDF
    let cmd = format!("https://www.servercontrolpanel.de/SCP/Logout?site_key={}",site_key);
    let res = client.get(&cmd)
    .header(header::REFERER, &referer).send()?;
    debug!("{:?}",&res);
    debug!("debug logout {}",res.text()?);

    Ok(())
}

#[derive(Deserialize)]
struct Config {
    pub url: String,
    pub server_id: String,
    pub user: String,
    pub password: String,
    pub cooldown_s: u64,
}