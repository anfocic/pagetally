use std::env;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct Config {
    pub bind_addr: String,
    pub database_url: String,
    pub allowed_sites: Option<Vec<String>>,
    pub email: Option<EmailConfig>,
}

#[derive(Clone, Debug)]
pub struct EmailConfig {
    pub resend_api_key: String,
    pub from: String,
    pub from_name: String,
    pub timeout: Duration,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("missing required env var: {0}")]
    Missing(&'static str),
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let database_url =
            env::var("DATABASE_URL").map_err(|_| ConfigError::Missing("DATABASE_URL"))?;

        let bind_addr = env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:3001".into());

        let allowed_sites = env::var("ALLOWED_SITES").ok().map(|s| {
            s.split(',')
                .map(|x| x.trim().to_string())
                .filter(|x| !x.is_empty())
                .collect()
        });

        let email = match (env::var("RESEND_API_KEY"), env::var("EMAIL_FROM")) {
            (Ok(api_key), Ok(from)) if !api_key.is_empty() && !from.is_empty() => {
                Some(EmailConfig {
                    resend_api_key: api_key,
                    from,
                    from_name: env::var("EMAIL_FROM_NAME").unwrap_or_else(|_| "pagetally".into()),
                    timeout: Duration::from_secs(10),
                })
            }
            _ => None,
        };

        Ok(Self {
            bind_addr,
            database_url,
            allowed_sites,
            email,
        })
    }
}
