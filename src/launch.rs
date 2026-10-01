use std::fmt;
use std::io::{self, IsTerminal};

use crate::cli::LaunchRequest;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LaunchOptions {
    pub desktop: bool,
    pub tui: bool,
    pub http: Option<crate::web::HttpConfig>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeEnvironment {
    pub desktop_available: bool,
    pub terminal_available: bool,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct NoUsableFrontend;

impl fmt::Debug for NoUsableFrontend {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

impl fmt::Display for NoUsableFrontend {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(
            "no usable frontend detected; specify --desktop, --tui, or --http explicitly",
        )
    }
}

impl std::error::Error for NoUsableFrontend {}

pub fn detect_environment() -> RuntimeEnvironment {
    RuntimeEnvironment {
        desktop_available: cclover_desktop::is_available(),
        terminal_available: io::stdin().is_terminal() && io::stdout().is_terminal(),
    }
}

pub fn resolve(
    request: LaunchRequest,
    environment: RuntimeEnvironment,
) -> Result<LaunchOptions, NoUsableFrontend> {
    match request {
        LaunchRequest::Explicit { desktop, tui, http } => Ok(LaunchOptions { desktop, tui, http }),
        LaunchRequest::Auto if environment.desktop_available => Ok(LaunchOptions {
            desktop: true,
            tui: false,
            http: None,
        }),
        LaunchRequest::Auto if environment.terminal_available => Ok(LaunchOptions {
            desktop: false,
            tui: true,
            http: None,
        }),
        LaunchRequest::Auto => Err(NoUsableFrontend),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::HttpConfig;

    #[test]
    fn auto_prefers_desktop_even_when_terminal_is_available() {
        assert_eq!(
            resolve(
                LaunchRequest::Auto,
                RuntimeEnvironment {
                    desktop_available: true,
                    terminal_available: true,
                },
            ),
            Ok(LaunchOptions {
                desktop: true,
                tui: false,
                http: None,
            })
        );
    }

    #[test]
    fn auto_uses_tui_when_desktop_is_unavailable() {
        assert_eq!(
            resolve(
                LaunchRequest::Auto,
                RuntimeEnvironment {
                    desktop_available: false,
                    terminal_available: true,
                },
            ),
            Ok(LaunchOptions {
                desktop: false,
                tui: true,
                http: None,
            })
        );
    }

    #[test]
    fn auto_fails_when_no_frontend_is_usable() {
        assert_eq!(
            resolve(
                LaunchRequest::Auto,
                RuntimeEnvironment {
                    desktop_available: false,
                    terminal_available: false,
                },
            ),
            Err(NoUsableFrontend)
        );
    }

    #[test]
    fn explicit_selection_bypasses_environment_policy() {
        let explicit = LaunchRequest::Explicit {
            desktop: false,
            tui: false,
            http: Some(HttpConfig::default()),
        };
        assert_eq!(
            resolve(
                explicit,
                RuntimeEnvironment {
                    desktop_available: false,
                    terminal_available: false,
                },
            ),
            Ok(LaunchOptions {
                desktop: false,
                tui: false,
                http: Some(HttpConfig::default()),
            })
        );
    }
}
