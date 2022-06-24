use std::{fmt, str::FromStr};

#[derive(Eq, PartialEq, Debug)]
pub enum Os {
    Linux,
    Mac,
    Ios,
    FreeBsd,
    Dragonfly,
    NetBsd,
    OpenBsd,
    Solaris,
    Android,
    Windows,
}

impl fmt::Display for Os {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Os::Linux => write!(f, "linux"),
            Os::Mac => write!(f, "macos"),
            Os::Ios => write!(f, "ios"),
            Os::FreeBsd => write!(f, "freebsd"),
            Os::Dragonfly => write!(f, "dragonfly"),
            Os::NetBsd => write!(f, "netbsd"),
            Os::OpenBsd => write!(f, "openbsd"),
            Os::Solaris => write!(f, "solaris"),
            Os::Android => write!(f, "android"),
            Os::Windows => write!(f, "windows"),
        }
    }
}

impl FromStr for Os {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "linux" => Ok(Os::Linux),
            "macos" => Ok(Os::Mac),
            "ios" => Ok(Os::Ios),
            "freebsd" => Ok(Os::FreeBsd),
            "dragonfly" => Ok(Os::Dragonfly),
            "netbsd" => Ok(Os::NetBsd),
            "openbsd" => Ok(Os::OpenBsd),
            "solaris" => Ok(Os::Solaris),
            "android" => Ok(Os::Android),
            "windows" => Ok(Os::Windows),
            _ => Err(format!("Unknown OS: {}", s)),
        }
    }
}
