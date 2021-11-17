use std::collections::HashMap;
use std::sync::Arc;

use ruma::api::client::r0::message::send_message_event;
use ruma::events::room::message::MessageEventContent;
use ruma::events::AnyMessageEventContent;
use ruma::{RoomId, UserId};
use serde::Deserialize;

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
async fn main() -> anyhow::Result<()> {
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
    let ruma_client = ruma::client::Client::with_http_client(
        http_client.clone(),
        config.base_url.clone(),
        Some(config.access_token.clone()),
    );

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
                    let txn_id = base32::encode(
                        base32::Alphabet::RFC4648 { padding: false },
                        &rand::random::<[u8; 8]>()[..],
                    );
                    let content =
                        AnyMessageEventContent::RoomMessage(MessageEventContent::text_plain(
                            format!("HEALTH CHECK {} FAILED: {}", health_check, e),
                        ));
                    let request = send_message_event::Request::new(&cfg.room_id, &txn_id, &content);
                    if let Err(e) = ruma_client.send_request_as(&cfg.user_id, request).await {
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
