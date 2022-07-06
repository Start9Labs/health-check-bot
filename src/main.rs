use std::collections::HashMap;
use std::sync::Arc;

use color_eyre::eyre::Error;
use ruma::api::client::message::send_message_event;
use ruma::events::room::message::RoomMessageEventContent;
use ruma::events::AnyMessageLikeEventContent;
use ruma::{OwnedRoomId, OwnedUserId, TransactionId};
use serde::Deserialize;

#[derive(Deserialize)]
struct Config {
    room_id: OwnedRoomId,
    access_token: String,
    user_id: OwnedUserId,
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
    let http_client = reqwest::Client::new();
    let ruma_client = ruma::client::Client::builder()
        .homeserver_url(config.base_url.clone())
        .access_token(Some(config.access_token.clone()))
        .http_client(http_client.clone())
        .await?;

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
            let http_client = http_client.clone();
            let ruma_client = ruma_client.clone();

            results.push(tokio::spawn(async move {
                if let Err(e) = http_client
                    .get(&health_check)
                    .send()
                    .await
                    .and_then(|res| res.error_for_status())
                {
                    let txn_id = TransactionId::new();
                    let content = AnyMessageLikeEventContent::RoomMessage(
                        RoomMessageEventContent::text_plain(format!(
                            "HEALTH CHECK {} FAILED: {}",
                            health_check, e
                        )),
                    );

                    if let Err(e) = async {
                        let request =
                            send_message_event::v3::Request::new(&cfg.room_id, &txn_id, &content)?;
                        let res = ruma_client.send_request_as(&cfg.user_id, request).await?;
                        Ok::<_, Error>(res)
                    }
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
