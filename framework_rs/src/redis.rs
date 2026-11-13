use std::time::Duration;

use redis::aio::{ConnectionManager, ConnectionManagerConfig};

use crate::exception;
use crate::exception::Exception;

pub const REDIS_UNREACHABLE: &str = "REDIS_UNREACHABLE";
pub const REDIS_URL_INVALID: &str = "REDIS_URL_INVALID";

pub async fn connect(url: &str) -> Result<ConnectionManager, Exception> {
    let client = redis::Client::open(url).map_err(|err| {
        exception!(
            code = REDIS_URL_INVALID,
            "redis url is not valid",
            source = err
        )
    })?;
    let mut conn = ConnectionManager::new_with_config(client, manager_config())
        .await
        .map_err(unreachable)?;
    let pong: String = redis::cmd("PING")
        .query_async(&mut conn)
        .await
        .map_err(unreachable)?;
    if pong != "PONG" {
        return Err(exception!(code = REDIS_UNREACHABLE, "cannot reach redis"));
    }
    Ok(conn)
}

// Fail fast when Redis is down instead of retrying for many seconds.
fn manager_config() -> ConnectionManagerConfig {
    ConnectionManagerConfig::new()
        .set_connection_timeout(Some(Duration::from_secs(2)))
        .set_response_timeout(Some(Duration::from_secs(5)))
        .set_number_of_retries(2)
        .set_max_delay(Duration::from_millis(500))
}

fn unreachable(err: redis::RedisError) -> Exception {
    exception!(code = REDIS_UNREACHABLE, "cannot reach redis", source = err)
}
