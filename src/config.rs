use std::time::Duration;
use std::string::String;
use std::str::FromStr;
use std::sync::Arc;
use iron::request::Request;
use crypto::sha2::Sha256;
use crypto::digest::Digest;
use rand::Rng;
use rand::os::OsRng;

pub struct Config {
    /// Duration before a pong packet after which to consider the connection
    /// closed (60 seconds)
    pub ping_timeout: Duration,
    /// Duration to wait before sending a new ping packet (25 seconds)
    pub ping_interval: Duration,
    /// Name of the HTTP cookie that contains the client sid to send as part
    /// of handshake response headers. Set to `None` to send a cookie.
    pub cookie: Option<String>,
    /// Path of the above cookie option. If `None`, no path will be sent, which
    /// means browsers will only send the cookie on the engine.io attached path
    /// (`Some("/engine.io")`). Set this to `Some("/")` to send the io cookie
    /// on all requests. (`None`)
    pub cookie_path: Option<String>,
    /// Generate a socket id. Takes an Iron `Request`, and returns the id String.
    /// Default value is `generate_id`
    pub generate_id: Arc<Box<Fn(&Request) -> String>>,
}

/// Default value of `generate_id`
pub fn generate_id(r: &Request) -> String {
    let mut hasher = Sha256::new();
    hasher.input_str(format!("{}{}", r.remote_addr, OsRng::new().unwrap().next_u32()).as_str());
    hasher.result_str()
}

impl Default for Config {
    fn default() -> Config {
        Config {
            ping_timeout: Duration::from_millis(60000),
            ping_interval: Duration::from_millis(25000),
            cookie: Some(String::from_str("io").unwrap()),
            cookie_path: None,
            generate_id: Arc::new(Box::new(generate_id)),
        }
    }
}
