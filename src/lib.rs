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
