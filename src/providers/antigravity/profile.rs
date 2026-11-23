//! Read the credentials written by agy ACP into an isolated GEMINI_HOME.

use std::{fs, io::ErrorKind, path::PathBuf};

use anyhow::{Context, Result, bail, ensure};
use chrono::{Duration, Utc};
use serde::Deserialize;

use super::{AntigravitySession, parse_credential_blob, session_is_stale};

pub(super) struct Profile {
    home: PathBuf,
    cached: Option<(AcpCredential, AntigravitySession)>,
}

#[derive(Deserialize, PartialEq, Eq)]
struct AcpCredential {
    client_id: String,
    client_secret: String,
    refresh_token: String,
    project_id: Option<String>,
}

impl AcpCredential {
    fn parse(contents: &[u8]) -> Result<Self> {
        let credential: Self = serde_json::from_slice(contents)
            .ok()
            .context("Invalid Antigravity ACP sign-in")?;
        ensure!(
            [
                &credential.client_id,
                &credential.client_secret,
                &credential.refresh_token
            ]
            .into_iter()
            .all(|value| !value.trim().is_empty()),
            "Incomplete Antigravity ACP sign-in"
        );
        Ok(credential)
    }

    fn refresh(&self, agent: &ureq::Agent) -> Result<AntigravitySession> {
        #[derive(Deserialize)]
        struct Grant {
            access_token: String,
            expires_in: i64,
        }

        let response = crate::platform::net::provider_response(
            agent
                .post("https://oauth2.googleapis.com/token")
                .send_form(&[
                    ("grant_type", "refresh_token"),
                    ("client_id", &self.client_id),
                    ("client_secret", &self.client_secret),
                    ("refresh_token", &self.refresh_token),
                ]),
            "Antigravity ACP sign-in",
            "Antigravity ACP sign-in was rejected; sign in again with this profile",
        )?;
        let text = response
            .into_string()
            .context("Read Antigravity OAuth response")?;
        let grant: Grant = serde_json::from_str(&text)
            .ok()
            .context("Invalid Antigravity OAuth response")?;
        let expires_at = Duration::try_seconds(grant.expires_in)
            .and_then(|duration| Utc::now().checked_add_signed(duration));
        ensure!(
            !grant.access_token.trim().is_empty() && !session_is_stale(expires_at),
            "Antigravity OAuth returned no usable access token"
        );
        Ok(AntigravitySession {
            access_token: grant.access_token,
            expires_at,
        })
    }
}

enum Credential {
    Cli(AntigravitySession),
    Acp(AcpCredential),
}

impl Profile {
    pub(super) fn new(home: PathBuf) -> Self {
        Self { home, cached: None }
    }

    fn credential(&self) -> Result<Credential> {
        ensure!(
            self.home.is_absolute(),
            "Choose an absolute Antigravity profile folder"
        );
        let directory = self.home.join("antigravity-acp");
        // agy 1.5 stores its sign-in separately from older ACP releases.
        // If both exist, the CLI login wins over a leftover ACP token.
        match fs::read(directory.join("antigravity-oauth-token")) {
            Ok(contents) => return Ok(Credential::Cli(parse_credential_blob(&contents)?)),
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(error).context("Read Antigravity CLI profile sign-in"),
        }
        match fs::read(directory.join("acp_token.json")) {
            Ok(contents) => Ok(Credential::Acp(AcpCredential::parse(&contents)?)),
            Err(error) if error.kind() == ErrorKind::NotFound => {
                bail!(
                    "Antigravity profile has no sign-in; use agy acp --login with this GEMINI_HOME or select a signed-in T3 Code profile"
                )
            }
            Err(error) => Err(error).context("Read Antigravity ACP profile sign-in"),
        }
    }

    pub(super) fn signed_in(&self) -> bool {
        self.credential().is_ok()
    }

    pub(super) fn session(
        &mut self,
        agent: &ureq::Agent,
    ) -> Result<(AntigravitySession, Option<&str>)> {
        match self.credential() {
            Ok(Credential::Acp(credential)) => {
                if self.cached.as_ref().is_none_or(|(cached, session)| {
                    *cached != credential || session_is_stale(session.expires_at)
                }) {
                    self.cached = None;
                    let session = credential.refresh(agent)?;
                    self.cached = Some((credential, session));
                }
                let (credential, session) = self
                    .cached
                    .as_ref()
                    .context("Missing Antigravity session")?;
                Ok((
                    session.clone(),
                    credential.project_id.as_deref().filter(|id| !id.is_empty()),
                ))
            }
            Ok(Credential::Cli(session)) => {
                self.cached = None;
                ensure!(
                    !session_is_stale(session.expires_at),
                    "Antigravity profile session expired; run agy acp with this profile to renew it"
                );
                Ok((session, None))
            }
            Err(error) => {
                self.cached = None;
                Err(error)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn cli_login_overrides_old_acp_credentials_and_stays_in_its_profile() {
        let temp = tempfile::tempdir().unwrap();
        let work = temp.path().join("work");
        let personal = temp.path().join("personal");
        let work_acp = work.join("antigravity-acp");
        let personal_acp = personal.join("antigravity-acp");
        fs::create_dir_all(&work_acp).unwrap();
        fs::create_dir_all(&personal_acp).unwrap();
        fs::write(
            work_acp.join("acp_token.json"),
            r#"{"client_id":"client","client_secret":"secret","refresh_token":"old-login"}"#,
        )
        .unwrap();
        let mut work = Profile::new(work);
        let mut personal = Profile::new(personal);
        assert!(matches!(work.credential(), Ok(Credential::Acp(_))));
        assert!(!personal.signed_in());

        let token_file = |directory: &std::path::Path, token: &str, expiry| {
            fs::write(
                directory.join("antigravity-oauth-token"),
                json!({"token": {"access_token": token, "expiry": expiry}}).to_string(),
            )
            .unwrap();
        };
        let expiry = (Utc::now() + Duration::hours(1)).timestamp();
        token_file(&work_acp, "work-token", expiry);
        token_file(&personal_acp, "personal-token", expiry);
        let agent = ureq::Agent::new();
        assert_eq!(work.session(&agent).unwrap().0.access_token, "work-token");
        assert_eq!(
            personal.session(&agent).unwrap().0.access_token,
            "personal-token"
        );
        token_file(&work_acp, "expired-token", 1);
        assert!(
            work.session(&agent)
                .err()
                .unwrap()
                .to_string()
                .contains("profile session expired")
        );
    }
}
