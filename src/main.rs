use std::collections::HashMap;
use std::convert::TryInto;
use std::sync::Arc;

use anyhow::{anyhow, Error};
use ruma::api::{OutgoingRequestAppserviceExt, SendAccessToken};
use ruma::events::room::message::MessageEventContent;
use ruma::events::AnyMessageEventContent;
use ruma::{RoomId, UserId};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
struct Config {
    room_id: RoomId,
    access_token: String,
    user_id: UserId,
    base_url: String,
    health_checks: Vec<String>,
    interval: f64,
    cooldown: usize,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let config: Config = serde_yaml::from_str(
        &tokio::fs::read_to_string(
            std::env::args()
                .skip(1)
                .next()
                .unwrap_or_else(|| "/etc/health-check-bot/config.yaml".to_owned()),
        )
        .await?,
    )?;
    let config = Arc::new(config);
    let mut cooldown_map: HashMap<String, usize> = config
        .health_checks
        .iter()
        .map(|h| (h.clone(), 0))
        .collect();
    loop {
        let mut results = Vec::with_capacity(cooldown_map.len());
        for (health_check, cooldown) in &mut cooldown_map {
            if *cooldown > 0 {
                *cooldown -= 1;
                continue;
            }
            let cfg = config.clone();
            let health_check = health_check.clone();
            results.push(tokio::spawn(async move {
                if let Err(e) = reqwest::get(&health_check)
                    .await
                    .and_then(|res| res.error_for_status())
                {
                    if let Err(e) = (|| async {
                        let res = reqwest::Client::new()
                            .execute(
                                ruma::api::client::r0::message::send_message_event::Request::new(
                                    &cfg.room_id,
                                    &base32::encode(
                                        base32::Alphabet::RFC4648 { padding: false },
                                        &rand::random::<[u8; 8]>()[..],
                                    ),
                                    &AnyMessageEventContent::RoomMessage(
                                        MessageEventContent::text_plain(format!(
                                            "HEALTH CHECK {} FAILED: {}",
                                            health_check, e
                                        )),
                                    ),
                                )
                                .try_into_http_request_with_user_id::<Vec<u8>>(
                                    &cfg.base_url,
                                    SendAccessToken::IfRequired(&cfg.access_token),
                                    cfg.user_id.clone(),
                                )?
                                .try_into()?,
                            )
                            .await?;
                        if let Err(e) = res.error_for_status_ref() {
                            return Err(anyhow!("{}: {}", e, res.json::<Value>().await?));
                        }
                        Ok::<(), Error>(())
                    })()
                    .await
                    {
                        eprintln!("ERROR SENDING MATRIX MESSAGE: {}", e);
                    }
                    Some(health_check)
                } else {
                    None
                }
            }));
        }
        for result in results {
            if let Some(check) = result.await.unwrap() {
                *cooldown_map.get_mut(&check).unwrap() = config.cooldown;
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs_f64(config.interval)).await;
    }
}
