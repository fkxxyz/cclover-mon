use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LaunchRequest {
    Auto,
    Explicit {
        desktop: bool,
        tui: bool,
        http: Option<HttpConfig>,
    },
}

pub(super) fn parse_launch_options(mut args: impl Iterator<Item = String>) -> LaunchRequest {
    let mut desktop = false;
    let mut tui = false;
    let mut http = false;
    let mut bind = DEFAULT_HTTP_BIND;
    let mut bind_explicit = false;
    let mut frontend_explicit = false;

    while let Some(option) = args.next() {
        match option.as_str() {
            "--desktop" => {
                if desktop {
                    fail("--desktop may only be specified once");
                }
                desktop = true;
                frontend_explicit = true;
            }
            "--tui" => {
                if tui {
                    fail("--tui may only be specified once");
                }
                tui = true;
                frontend_explicit = true;
            }
            "--http" => {
                if http {
                    fail("--http may only be specified once");
                }
                http = true;
                frontend_explicit = true;
            }
            "--http-bind" => {
                if bind_explicit {
                    fail("--http-bind may only be specified once");
                }
                let value = args
                    .next()
                    .unwrap_or_else(|| fail("--http-bind requires an IP:port value"));
                bind = value.parse::<SocketAddr>().unwrap_or_else(|_| {
                    fail("--http-bind requires an IP:port value such as 0.0.0.0:9847")
                });
                bind_explicit = true;
            }
            other => fail(&format!("unknown launch option: {other}")),
        }
    }

    if bind_explicit && !http {
        fail("--http-bind requires --http");
    }
    if http && !cfg!(feature = "http") {
        fail("--http is unavailable in this build; rebuild with the `http` feature");
    }

    if frontend_explicit {
        LaunchRequest::Explicit {
            desktop,
            tui,
            http: http.then_some(HttpConfig { bind }),
        }
    } else {
        LaunchRequest::Auto
    }
}
