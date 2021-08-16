use std::convert::TryInto;

use ruma::{
    api::{OutgoingRequestAppserviceExt, SendAccessToken},
    events::{room::message::MessageEventContent, AnyMessageEventContent},
    RoomId, UserId,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct Config {
    room_id: RoomId,
    access_token: String,
    user_id: UserId,
    base_url: String,
    health_checks: Vec<String>,
    interval: f64,
}

fn main() {
    let config: Config = toml::from_str(
        &std::fs::read_to_string(
            std::env::args()
                .skip(1)
                .next()
                .unwrap_or_else(|| "/etc/health-check-bot/config.toml".to_owned()),
        )
        .unwrap(),
    )
    .unwrap();
    loop {
        for health_check in &config.health_checks {
            if let Err(e) =
                reqwest::blocking::get(health_check).and_then(|res| res.error_for_status())
            {
                reqwest::blocking::Client::new()
                    .execute(
                        ruma::api::client::r0::message::send_message_event::Request::new(
                            &config.room_id,
                            &base32::encode(
                                base32::Alphabet::RFC4648 { padding: false },
                                &rand::random::<[u8; 8]>()[..],
                            ),
                            &AnyMessageEventContent::RoomMessage(MessageEventContent::text_plain(
                                format!("HEALTH CHECK {} FAILED: {}", health_check, e),
                            )),
                        )
                        .try_into_http_request_with_user_id::<Vec<u8>>(
                            &config.base_url,
                            SendAccessToken::IfRequired(&config.access_token),
                            config.user_id.clone(),
                        )
                        .unwrap()
                        .try_into()
                        .unwrap(),
                    )
                    .unwrap();
            }
            std::thread::sleep(std::time::Duration::from_secs_f64(config.interval))
        }
    }
}
