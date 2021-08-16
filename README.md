# health-check-bot

## Installing

- Assumes [yq](https://mikefarah.github.io/yq) is installed.
- Fill in `$HS_DIR` with the data directory for your synapse homeserver
- Fill in `$HS_ADDR` with your homeserver address (i.e. `matrix.org`)
- Create a room for health check messages and set `$HEALTH_ROOM_ID` to its id (i.e. `!asdfghjkl:matrix.org`)
- Have @_health_check_bot join the room

```sh
git clone git@github.com:Start9Labs/health-check-bot.git
cd health-check-bot
cp health_check_bot.yaml $HS_DIR
yq e -i ".as_token = \"$(cat /dev/urandom | head -c32 | xxd -p -c32\")" $HS_DIR/health_check_bot.yaml
yq e -i ".hs_token = \"$(cat /dev/urandom | head -c32 | xxd -p -c32\")" $HS_DIR/health_check_bot.yaml
yq e -i ".app_service_config_files += [\"$HS_DIR/health_check_bot.yaml\"]" $HS_DIR/homeserver.yaml
mkdir /etc/health-check-bot
cp config-sample.yaml /etc/health-check-bot/config.yaml
yq e -i ".room_id = \"$HEALTH_ROOM_ID\"" /etc/health-check-bot/config.yaml
yq e -i ".access_token = \"$(yq e ".as_token" $HS_DIR/health_check_bot.yaml)\"" /etc/health-check-bot/config.yaml
yq e -i ".user_id = \"@_health_check_bot:$HS_ADDR\"" /etc/health-check-bot/config.yaml
yq e -i ".base_url = \"https://$HS_ADDR\"" /etc/health-check-bot/config.yaml
cp health-check-bot.service /etc/systemd/system
systemctl enable heath-check-bot.service
```
- restart synapse
- add health check urls to `/etc/health-check-bot/config.yaml`
  - the bot will perform a GET to each of these urls and post to the channel if there are any errors / non-2xx responses
- `systemctl start health-check-bot.service`