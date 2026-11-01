//! Who's using the web app. The gateway doesn't sign people in itself: a proxy in front of it
//! does, e.g. Google Cloud's Identity-Aware Proxy, and passes on who they are with each request.
//! Servers created from the web app belong to whoever created them, and can be deleted by them
//! or by admins, the people Kubernetes RBAC lets delete QuakeServers.

use crate::config::{AuthConfig, AuthMode};
use crate::prelude::*;
use anyhow::Context as _;
use axum::http::HeaderMap;
use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation};
use k8s_openapi::api::authorization::v1::{
    ResourceAttributes, SubjectAccessReview, SubjectAccessReviewSpec,
};
use kube::api::PostParams;
use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tracing::*;

/// The header IAP puts its signed assertion in.
const IAP_HEADER: &str = "x-goog-iap-jwt-assertion";
const IAP_ISSUER: &str = "https://cloud.google.com/iap";
const IAP_KEYS_URL: &str = "https://www.gstatic.com/iap/verify/public_key-jwk";

/// IAP's keys are fetched again when an assertion is signed by one that isn't known, but not
/// more often than this.
const IAP_KEYS_REFRESH: Duration = Duration::from_secs(60);

/// How long a browser has to open a WebTransport session after loading the play page. Clients
/// that reconnect later fall back to the WebSocket.
const TICKET_LIFETIME: Duration = Duration::from_secs(60 * 60);

/// How long whether someone is an admin is remembered, so listing servers doesn't ask the API
/// server every time.
const ADMIN_CACHE: Duration = Duration::from_secs(60);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct User {
    pub email: String,
    pub groups: Vec<String>,
}

pub struct Auth {
    config: AuthConfig,
    client: Client,
    http: reqwest::Client,
    iap_keys: RwLock<Option<(JwkSet, Instant)>>,
    ticket_key: Vec<u8>,
    admins: RwLock<HashMap<(String, String), (bool, Instant)>>,
}

#[derive(Deserialize)]
struct IapClaims {
    email: String,
    aud: String,
}

#[derive(Serialize, Deserialize)]
struct TicketClaims {
    /// The server, as <namespace>/<name>.
    sub: String,
    exp: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("not signed in")]
    Missing,
    #[error("invalid assertion: {0}")]
    Invalid(#[from] anyhow::Error),
}

impl Auth {
    pub fn new(config: AuthConfig, client: Client) -> anyhow::Result<Self> {
        if config.mode == AuthMode::Iap
            && config.iap.audience.is_none()
            && config.iap.project_number.is_none()
        {
            anyhow::bail!("IAP needs an audience or a project number to check assertions against");
        }
        let ticket_key = match &config.ticket_key_file {
            Some(path) => std::fs::read(path)
                .with_context(|| format!("reading ticket key {}", path.display()))?,
            None => rand::random::<[u8; 32]>().to_vec(),
        };
        Ok(Auth {
            config,
            client,
            http: reqwest::Client::new(),
            iap_keys: RwLock::new(None),
            ticket_key,
            admins: RwLock::new(HashMap::new()),
        })
    }

    /// Whether the gateway knows who people are. Without it, anyone can use the web app and
    /// servers have no owners.
    pub fn enabled(&self) -> bool {
        self.config.mode != AuthMode::None
    }

    /// Who made a request, or None when the gateway doesn't know who people are.
    pub async fn user(&self, headers: &HeaderMap) -> Result<Option<User>, AuthError> {
        let header = |name: &str| {
            headers
                .get(name)
                .and_then(|value| value.to_str().ok())
                .map(str::trim)
                .filter(|value| !value.is_empty())
        };
        match self.config.mode {
            AuthMode::None => Ok(None),
            AuthMode::Iap => {
                let assertion = header(IAP_HEADER).ok_or(AuthError::Missing)?;
                Ok(Some(self.verify_iap(assertion).await?))
            }
            AuthMode::Header => {
                let email = header(&self.config.header.user).ok_or(AuthError::Missing)?;
                let groups = self
                    .config
                    .header
                    .groups
                    .as_deref()
                    .and_then(header)
                    .map(|groups| {
                        groups
                            .split(',')
                            .map(str::trim)
                            .filter(|group| !group.is_empty())
                            .map(String::from)
                            .collect()
                    })
                    .unwrap_or_default();
                Ok(Some(User {
                    email: email.to_owned(),
                    groups,
                }))
            }
        }
    }

    async fn verify_iap(&self, assertion: &str) -> anyhow::Result<User> {
        let header = jsonwebtoken::decode_header(assertion)?;
        let kid = header.kid.context("no key ID")?;
        let key = self.iap_key(&kid).await?;
        let mut validation = Validation::new(Algorithm::ES256);
        validation.set_issuer(&[IAP_ISSUER]);
        validation.set_required_spec_claims(&["exp", "iat", "iss", "aud"]);
        // Checked below, since the audience may only be known by its project.
        validation.validate_aud = false;
        let claims = jsonwebtoken::decode::<IapClaims>(assertion, &key, &validation)?.claims;
        if !self.iap_audience_matches(&claims.aud) {
            anyhow::bail!("unexpected audience {}", claims.aud);
        }
        Ok(User {
            email: claims.email,
            groups: Vec::new(),
        })
    }

    fn iap_audience_matches(&self, aud: &str) -> bool {
        match (&self.config.iap.audience, &self.config.iap.project_number) {
            (Some(audience), _) => aud == audience,
            (None, Some(project)) => aud.starts_with(&format!("/projects/{project}/")),
            (None, None) => false,
        }
    }

    async fn iap_key(&self, kid: &str) -> anyhow::Result<DecodingKey> {
        let fetched = {
            let keys = self.iap_keys.read().unwrap();
            if let Some((set, fetched)) = keys.as_ref() {
                if let Some(jwk) = set.find(kid) {
                    return Ok(DecodingKey::from_jwk(jwk)?);
                }
                Some(*fetched)
            } else {
                None
            }
        };
        if fetched.is_some_and(|fetched| fetched.elapsed() < IAP_KEYS_REFRESH) {
            anyhow::bail!("unknown key {kid}");
        }
        let set: JwkSet = self
            .http
            .get(IAP_KEYS_URL)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
            .context("fetching IAP's keys")?;
        let key = set.find(kid).map(DecodingKey::from_jwk).transpose()?;
        *self.iap_keys.write().unwrap() = Some((set, Instant::now()));
        key.with_context(|| format!("unknown key {kid}"))
    }

    /// A ticket that lets a browser open a WebTransport session to a server. Sessions don't go
    /// through the proxy in front of the web app, so the play page hands one out.
    pub fn ticket(&self, namespace: &str, name: &str) -> String {
        let exp = (SystemTime::now() + TICKET_LIFETIME)
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let claims = TicketClaims {
            sub: format!("{namespace}/{name}"),
            exp,
        };
        jsonwebtoken::encode(
            &Header::new(Algorithm::HS256),
            &claims,
            &EncodingKey::from_secret(&self.ticket_key),
        )
        .expect("signing with HS256 can't fail")
    }

    pub fn check_ticket(&self, ticket: &str, namespace: &str, name: &str) -> bool {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.sub = Some(format!("{namespace}/{name}"));
        validation.set_required_spec_claims(&["exp", "sub"]);
        jsonwebtoken::decode::<TicketClaims>(
            ticket,
            &DecodingKey::from_secret(&self.ticket_key),
            &validation,
        )
        .is_ok()
    }

    /// Whether the user can delete anyone's servers in the namespace, which is whether
    /// Kubernetes RBAC lets them delete QuakeServers there.
    pub async fn is_admin(&self, user: &User, namespace: &str) -> bool {
        let key = (user.email.clone(), namespace.to_owned());
        if let Some((admin, checked)) = self.admins.read().unwrap().get(&key) {
            if checked.elapsed() < ADMIN_CACHE {
                return *admin;
            }
        }
        let review = SubjectAccessReview {
            spec: SubjectAccessReviewSpec {
                user: Some(user.email.clone()),
                groups: (!user.groups.is_empty()).then(|| user.groups.clone()),
                resource_attributes: Some(ResourceAttributes {
                    namespace: Some(namespace.to_owned()),
                    verb: Some("delete".to_owned()),
                    group: Some("quake.kube".to_owned()),
                    resource: Some("quakeservers".to_owned()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..Default::default()
        };
        let result = Api::<SubjectAccessReview>::all(self.client.clone())
            .create(&PostParams::default(), &review)
            .await;
        match result {
            Ok(review) => {
                let admin = review.status.is_some_and(|status| status.allowed);
                let mut admins = self.admins.write().unwrap();
                admins.retain(|_, (_, checked)| checked.elapsed() < ADMIN_CACHE);
                admins.insert(key, (admin, Instant::now()));
                admin
            }
            Err(error) => {
                warn!(user = %user.email, "checking whether user is an admin failed: {error}");
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::IapConfig;

    fn auth(config: AuthConfig) -> Auth {
        // Never contacted by these tests.
        let client =
            Client::try_from(kube::Config::new("http://127.0.0.1:1".parse().unwrap())).unwrap();
        Auth::new(config, client).unwrap()
    }

    #[tokio::test]
    async fn tickets_are_for_one_server() {
        let auth = auth(AuthConfig::default());
        let ticket = auth.ticket("quake", "duel");
        assert!(auth.check_ticket(&ticket, "quake", "duel"));
        assert!(!auth.check_ticket(&ticket, "quake", "ffa"));
        assert!(!auth.check_ticket(&ticket, "other", "duel"));
        assert!(!auth.check_ticket("not-a-ticket", "quake", "duel"));
        // Another replica's key.
        let other = super::tests::auth(AuthConfig::default());
        assert!(!other.check_ticket(&ticket, "quake", "duel"));
    }

    #[tokio::test]
    async fn iap_audience() {
        let iap = |audience: Option<&str>, project_number: Option<&str>| {
            auth(AuthConfig {
                mode: AuthMode::Iap,
                iap: IapConfig {
                    audience: audience.map(String::from),
                    project_number: project_number.map(String::from),
                },
                ..Default::default()
            })
        };
        let exact = iap(Some("/projects/1/global/backendServices/2"), None);
        assert!(exact.iap_audience_matches("/projects/1/global/backendServices/2"));
        assert!(!exact.iap_audience_matches("/projects/1/global/backendServices/3"));
        let project = iap(None, Some("1"));
        assert!(project.iap_audience_matches("/projects/1/global/backendServices/3"));
        assert!(!project.iap_audience_matches("/projects/12/global/backendServices/3"));
        let client =
            Client::try_from(kube::Config::new("http://127.0.0.1:1".parse().unwrap())).unwrap();
        let unchecked = AuthConfig {
            mode: AuthMode::Iap,
            ..Default::default()
        };
        assert!(Auth::new(unchecked, client).is_err());
    }

    #[tokio::test]
    async fn header_users() {
        let auth = auth(AuthConfig {
            mode: AuthMode::Header,
            header: crate::config::HeaderAuthConfig {
                user: "X-Forwarded-Email".to_owned(),
                groups: Some("X-Forwarded-Groups".to_owned()),
            },
            ..Default::default()
        });
        let mut headers = HeaderMap::new();
        assert!(matches!(auth.user(&headers).await, Err(AuthError::Missing)));
        headers.insert("x-forwarded-email", "ranger@example.com".parse().unwrap());
        headers.insert("x-forwarded-groups", "admins, players,".parse().unwrap());
        assert_eq!(
            auth.user(&headers).await.unwrap(),
            Some(User {
                email: "ranger@example.com".to_owned(),
                groups: vec!["admins".to_owned(), "players".to_owned()],
            })
        );
    }

    /// Signs IAP assertions with a key the gateway already has, as if fetched from Google.
    fn iap_signer(auth: &Auth) -> impl Fn(serde_json::Value) -> String {
        use p256::pkcs8::EncodePrivateKey;
        let secret = p256::SecretKey::random(&mut rand_core::OsRng);
        let mut jwk: serde_json::Value =
            serde_json::from_str(&secret.public_key().to_jwk_string()).unwrap();
        jwk["kid"] = "test-key".into();
        jwk["alg"] = "ES256".into();
        let set = serde_json::from_value(serde_json::json!({ "keys": [jwk] })).unwrap();
        *auth.iap_keys.write().unwrap() = Some((set, Instant::now()));
        let key = EncodingKey::from_ec_der(secret.to_pkcs8_der().unwrap().as_bytes());
        move |claims| {
            let mut header = Header::new(Algorithm::ES256);
            header.kid = Some("test-key".to_owned());
            jsonwebtoken::encode(&header, &claims, &key).unwrap()
        }
    }

    #[tokio::test]
    async fn iap_assertions() {
        let auth = auth(AuthConfig {
            mode: AuthMode::Iap,
            iap: IapConfig {
                project_number: Some("1".to_owned()),
                ..Default::default()
            },
            ..Default::default()
        });
        let sign = iap_signer(&auth);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let claims = |iss: &str, aud: &str, exp: u64| {
            serde_json::json!({
                "iss": iss,
                "aud": aud,
                "exp": exp,
                "iat": now,
                "sub": "accounts.google.com:1",
                "email": "ranger@example.com",
            })
        };
        let user = |assertion: String| {
            let mut headers = HeaderMap::new();
            headers.insert(IAP_HEADER, assertion.parse().unwrap());
            let auth = &auth;
            async move { auth.user(&headers).await }
        };
        let audience = "/projects/1/global/backendServices/2";
        assert_eq!(
            user(sign(claims(IAP_ISSUER, audience, now + 600)))
                .await
                .unwrap()
                .unwrap()
                .email,
            "ranger@example.com"
        );
        // Another project's app, another issuer, and an expired assertion.
        for claims in [
            claims(
                IAP_ISSUER,
                "/projects/2/global/backendServices/2",
                now + 600,
            ),
            claims("https://example.com", audience, now + 600),
            claims(IAP_ISSUER, audience, now - 600),
        ] {
            assert!(matches!(
                user(sign(claims)).await,
                Err(AuthError::Invalid(_))
            ));
        }
        // A key IAP didn't sign with. The keys were just fetched, so they aren't again.
        let other = iap_signer(&super::tests::auth(AuthConfig::default()));
        assert!(matches!(
            user(other(claims(IAP_ISSUER, audience, now + 600))).await,
            Err(AuthError::Invalid(_))
        ));
    }

    #[tokio::test]
    async fn iap_needs_an_assertion() {
        let auth = auth(AuthConfig {
            mode: AuthMode::Iap,
            iap: IapConfig {
                project_number: Some("1".to_owned()),
                ..Default::default()
            },
            ..Default::default()
        });
        let mut headers = HeaderMap::new();
        // The header oauth2-proxy uses isn't trusted.
        headers.insert("x-forwarded-email", "ranger@example.com".parse().unwrap());
        assert!(matches!(auth.user(&headers).await, Err(AuthError::Missing)));
        headers.insert(IAP_HEADER, "not.an.assertion".parse().unwrap());
        assert!(matches!(
            auth.user(&headers).await,
            Err(AuthError::Invalid(_))
        ));
    }
}
