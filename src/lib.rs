#[macro_use]
extern crate iron;
extern crate rand;
extern crate url;
extern crate rustc_serialize as serialize;
extern crate crypto;
extern crate modifier;
extern crate cookie;

pub mod packet;
pub mod server;
pub mod socket;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {}
}
