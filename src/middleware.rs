use anyhow::anyhow;

use poem::{
    async_trait,
    error::{BadRequest, InternalServerError},
    handler,
    http::{HeaderMap, StatusCode},
    post, Endpoint, EndpointExt, Error, IntoResponse, Middleware, Request, Response, Result, Route,
};

use octocrab::models::webhook_events::{WebhookEvent, WebhookEventPayload, WebhookEventType};

pub struct GithubWebhook {
    secret: String,
}

impl GithubWebhook {
    pub fn new(secret: &str) -> Self {
        GithubWebhook {
            secret: secret.into(),
        }
    }

    pub fn route(&self) -> GithubWebhookImpl<Route> {
        Route::new().at("/", post(dispatch)).with(self)
    }
}

impl<E: Endpoint> Middleware<E> for &GithubWebhook {
    type Output = GithubWebhookImpl<E>;

    fn transform(&self, ep: E) -> Self::Output {
        GithubWebhookImpl(ep, self.secret.clone())
    }
}

pub struct GithubWebhookImpl<E>(E, String);

#[async_trait]
impl<E: Endpoint> Endpoint for GithubWebhookImpl<E> {
    type Output = Response;

    async fn call(&self, mut req: Request) -> Result<Self::Output> {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        if let Some(value) = req.header("X-Hub-Signature-256") {
            let encoded_sig = value.to_owned();
            let signature = match encoded_sig.as_str().strip_prefix("sha256=") {
                Some(hex) => match hex::decode(hex) {
                    Ok(hex) => hex,
                    Err(err) => {
                        tracing::warn!("Failed to hex decode Github's signature: {}", err);
                        return Ok(Response::builder().status(StatusCode::BAD_REQUEST).finish());
                    }
                },
                None => {
                    tracing::warn!("Failed to verify Github's signature: Unexpected format");
                    return Ok(Response::builder().status(StatusCode::BAD_REQUEST).finish());
                }
            };
            let mut mac: Hmac<Sha256> =
                Hmac::new_from_slice(self.1.as_bytes()).map_err(InternalServerError)?;
            let body = req.take_body().into_bytes().await.map_err(BadRequest)?;
            mac.update(&body);
            req.set_body(body);
            if let Err(err) = mac.verify_slice(&signature) {
                tracing::warn!("Failed to verify Github's signature: {}", err);
                return Ok(Response::builder().status(StatusCode::BAD_REQUEST).finish());
            } else {
                let res = self.0.call(req).await;
                res.map(|resp| resp.into_response())
            }
        } else {
            tracing::warn!("Event not signed but webhook secret configured, ignoring event");
            return Ok(Response::builder().status(StatusCode::BAD_REQUEST).finish());
        }
    }
}

#[handler]
fn dispatch(headers: &HeaderMap, body: String) -> Result<String> {
    fn inner(
        headers: &HeaderMap,
        body: String,
    ) -> std::result::Result<String, Box<dyn std::error::Error>> {
        let event_type = headers.get("X-Github-Event").ok_or("missing header")?;
        let event_type = event_type.to_str()?;
        tracing::info!("event_type: {event_type}");

        let event = WebhookEvent::try_from_header_and_body(event_type, &body)?;
        match event.specific {
            WebhookEventPayload::Ping(ping_event) => tracing::info!("got ping"),
            WebhookEventPayload::IssueComment(issue_comment) => {
                tracing::info!("got issue comment {:?}", issue_comment.action)
            }
            WebhookEventPayload::Issues(issues) => {
                tracing::info!("got issue {:?}", issues.action)
            }
            WebhookEventPayload::PullRequest(pull_request) => {
                tracing::info!("got PR {:?}", pull_request.action)
            }
            WebhookEventPayload::Push(push) => {
                tracing::info!("got push to {:?}", push.r#ref)
            }
            _ => {
                tracing::warn!("got unhandled payload for {}", event_type);
                return Err(anyhow!("unhandled event type").into());
            }
        }

        Ok("OK".into())
    }

    inner(headers, body).map_err(|err| {
        tracing::warn!("error: {err}");
        Error::from_status(StatusCode::BAD_REQUEST)
    })
}
