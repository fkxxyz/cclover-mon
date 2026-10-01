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

pub(super) fn parse_launch_options(
    mut args: impl Iterator<Item = String>,
) -> Result<LaunchRequest, CliError> {
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
                    return Err(CliError::new("--desktop may only be specified once"));
                }
                desktop = true;
                frontend_explicit = true;
            }
            "--tui" => {
                if tui {
                    return Err(CliError::new("--tui may only be specified once"));
                }
                tui = true;
                frontend_explicit = true;
            }
            "--http" => {
                if http {
                    return Err(CliError::new("--http may only be specified once"));
                }
                http = true;
                frontend_explicit = true;
            }
            "--http-bind" => {
                if bind_explicit {
                    return Err(CliError::new("--http-bind may only be specified once"));
                }
                let value = args
                    .next()
                    .ok_or_else(|| CliError::new("--http-bind requires an IP:port value"))?;
                bind = value.parse::<SocketAddr>().map_err(|_| {
                    CliError::new("--http-bind requires an IP:port value such as 0.0.0.0:9847")
                })?;
                bind_explicit = true;
            }
            other => return Err(CliError::new(format!("unknown launch option: {other}"))),
        }
    }

    if bind_explicit && !http {
        return Err(CliError::new("--http-bind requires --http"));
    }
    if http && !cfg!(feature = "http") {
        return Err(CliError::new(
            "--http is unavailable in this build; rebuild with the `http` feature",
        ));
    }

    if frontend_explicit {
        Ok(LaunchRequest::Explicit {
            desktop,
            tui,
            http: http.then_some(HttpConfig { bind }),
        })
    } else {
        Ok(LaunchRequest::Auto)
    }
}
