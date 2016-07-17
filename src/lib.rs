//! ## Hello World
//! ```no_run
//! extern crate engine_io;
//! extern crate iron;
//!
//! use iron::prelude::*;
//! use engine_io::server::Server;
//!
//! fn main() {
//!     let s = Server::new();
//!     s.on_connection(|so| {
//!         println!("connected to {}", so.id());
//!         so.on_message(|m| {
//!             println!("message: {}", String::from_utf8(m).unwrap());
//!         });
//!         so.send("Hello, world!")
//!     });
//!
//!     println!("listening");
//!     Iron::new(s).http("localhost:3000").unwrap();
//! }
//!```

#[macro_use]
extern crate iron;
extern crate rand;
extern crate url;
extern crate rustc_serialize as serialize;
extern crate crypto;
extern crate modifier;
extern crate cookie;
extern crate time;
extern crate hyper;
#[macro_use]
extern crate log;

pub mod packet;
pub mod server;
pub mod socket;
pub mod config;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {}
}
