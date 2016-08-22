extern crate futures;
extern crate futures_cpupool;

use std::io;
use std::net::{IpAddr, SocketAddr, ToSocketAddrs};
use std::str::FromStr;

use futures::{BoxFuture, Future};
use futures_cpupool::CpuPool;

/// The Resolver trait represents an object capable of
/// resolving host names into IP addresses.
pub trait Resolver {
    /// Given a host name, this function returns a Future which
    /// will eventually resolve into a list of IP addresses.
    fn resolve(&self, host: &str) -> BoxFuture<Vec<IpAddr>, io::Error>;
}

/// A resolver based on a thread pool.
///
/// This resolver uses the `ToSocketAddrs` trait inside
/// a thread to provide non-blocking address resolving.
#[derive(Clone)]
pub struct CpuPoolResolver {
    pool: CpuPool,
}

impl CpuPoolResolver {
    /// Create a new CpuPoolResolver with the given number of threads.
    pub fn new(num_threads: u32) -> Self {
        CpuPoolResolver {
            pool: CpuPool::new(num_threads),
        }
    }
}

impl Resolver for CpuPoolResolver {
    fn resolve(&self, host: &str) -> BoxFuture<Vec<IpAddr>, io::Error> {
        let host = format!("{}:0", host);
        self.pool.execute(move || {
            match host[..].to_socket_addrs() {
                Ok(it) => Ok(it.map(|s| s.ip()).collect()),
                Err(e) => Err(e),
            }
        }).then(|res| {
            // CpuFuture cannot fail unless it panics
            res.unwrap()
        }).boxed()
    }
}

/// An Endpoint is a way of identifying the target of a connection.
///
/// It can be a socket address or a host name which needs to be resolved
/// into a list of IP addresses.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Endpoint<'a> {
    Host(&'a str, u16),
    SocketAddr(SocketAddr),
}

/// A trait for objects that can be converted into an Endpoint.
///
/// This trait is implemented for the following types:
///
/// * `SocketAddr`, `&SocketAddr` - a socket address.
/// * `(IpAddr, u16)`, `(&str, u16)` - a target and a port.
/// * `&str` - a string formatted as `<target>:<port>` where
/// `<target>` is a host name or an IP address.
///
/// This trait is similar to the `ToSocketAddrs` trait, except
/// that it does not perform host name resolution.
pub trait ToEndpoint<'a> {
    fn to_endpoint(self) -> io::Result<Endpoint<'a>>;
}

impl<'a> ToEndpoint<'a> for SocketAddr {
    fn to_endpoint(self) -> io::Result<Endpoint<'a>> {
        Ok(Endpoint::SocketAddr(self))
    }
}

impl<'a, 'b> ToEndpoint<'a> for &'b SocketAddr {
    fn to_endpoint(self) -> io::Result<Endpoint<'a>> {
        Ok(Endpoint::SocketAddr(*self))
    }
}

impl <'a> ToEndpoint<'a> for (IpAddr, u16) {
    fn to_endpoint(self) -> io::Result<Endpoint<'a>> {
        Ok(Endpoint::SocketAddr(SocketAddr::new(self.0, self.1)))
    }
}

impl<'a> ToEndpoint<'a> for (&'a str, u16) {
    fn to_endpoint(self) -> io::Result<Endpoint<'a>> {
        match IpAddr::from_str(self.0) {
            Ok(addr) => (addr, self.1).to_endpoint(),
            Err(_) => Ok(Endpoint::Host(self.0, self.1)),
        }
    }
}

impl<'a> ToEndpoint<'a> for &'a str {
    fn to_endpoint(self) -> io::Result<Endpoint<'a>> {
        fn parse_port(port: &str) -> io::Result<u16> {
            u16::from_str(port)
                .map_err(|_| io::Error::new(io::ErrorKind::Other, "invalid port"))
        }

        match self.rfind(":") {
            Some(idx) => {
                let host = &self[..idx];
                let port = try!(parse_port(&self[idx+1..]));
                (host, port).to_endpoint()
            }
            None => {
                Err(io::Error::new(io::ErrorKind::Other, "invalid endpoint"))
            }
        }
    }
}

#[test]
fn test_endpoint_str_port() {
    use std::net::Ipv4Addr;

    let ep = "0.0.0.0:1227".to_endpoint().unwrap();
    match ep {
        Endpoint::SocketAddr(addr) => {
            assert_eq!(addr.ip(), IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)));
            assert_eq!(addr.port(), 1227);
        }
        _ => panic!(),
    }
}

#[test]
fn test_endpoint_str() {
    let ep = "localhost:1227".to_endpoint().unwrap();
    match ep {
        Endpoint::Host(host, port) => {
            assert_eq!(host, "localhost");
            assert_eq!(port, 1227);
        }
        _ => panic!(),
    }
}
