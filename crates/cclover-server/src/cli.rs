use std::net::SocketAddr;

use cclover_http::{DEFAULT_HTTP_BIND, HttpConfig};

pub enum Command {
    Run(HttpConfig),
    Service(HttpConfig),
    Help,
}

pub fn parse() -> Result<Command, String> {
    parse_args(std::env::args().skip(1))
}

fn parse_args(mut args: impl Iterator<Item = String>) -> Result<Command, String> {
    let mut bind = DEFAULT_HTTP_BIND;
    let mut bind_seen = false;
    let mut service = false;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bind" => {
                if bind_seen {
                    return Err("--bind may only be specified once".to_owned());
                }
                let value = args
                    .next()
                    .ok_or_else(|| "--bind requires an IP:port value".to_owned())?;
                bind = value.parse::<SocketAddr>().map_err(|_| {
                    "--bind requires an IP:port value such as 0.0.0.0:9847".to_owned()
                })?;
                bind_seen = true;
            }
            "--service" => {
                if service {
                    return Err("--service may only be specified once".to_owned());
                }
                service = true;
            }
            "--help" | "-h" | "help" => {
                if args.next().is_some() || bind_seen || service {
                    return Err("--help cannot be combined with other arguments".to_owned());
                }
                return Ok(Command::Help);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    let config = HttpConfig { bind };
    Ok(if service {
        Command::Service(config)
    } else {
        Command::Run(config)
    })
}

pub fn print_help() {
    println!(
        "cclover-mon-server\n\nUSAGE:\n    cclover-mon-server [--bind <ip:port>] [--service]\n\nOPTIONS:\n    --bind <ip:port>  HTTP listen address (default 127.0.0.1:9847)\n    --service         Run under Windows Service Control Manager\n    -h, --help        Show this help"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(values: &[&str]) -> Result<Command, String> {
        parse_args(values.iter().map(|value| (*value).to_owned()))
    }

    #[test]
    fn defaults_to_loopback_http_server() {
        let Command::Run(config) = parse(&[]).unwrap() else {
            panic!("expected run command");
        };
        assert_eq!(config.bind, DEFAULT_HTTP_BIND);
    }

    #[test]
    fn accepts_explicit_bind() {
        let Command::Run(config) = parse(&["--bind", "0.0.0.0:9847"]).unwrap() else {
            panic!("expected run command");
        };
        assert_eq!(config.bind, "0.0.0.0:9847".parse().unwrap());
    }

    #[test]
    fn service_mode_preserves_bind_configuration() {
        let Command::Service(config) = parse(&["--service", "--bind", "127.0.0.1:9000"]).unwrap()
        else {
            panic!("expected service command");
        };
        assert_eq!(config.bind, "127.0.0.1:9000".parse().unwrap());
    }
}
