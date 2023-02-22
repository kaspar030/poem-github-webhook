use anyhow::anyhow;
use poem::{
    async_trait,
    error::{BadRequest, GetDataError, InternalServerError},
    handler,
    http::{request, HeaderMap, StatusCode},
    post,
    web::Json,
    Body, Endpoint, EndpointExt, Error, IntoResponse, Middleware, Request, Response, Result, Route,
};

use crate::payload::{IssueCommentPayload, Payload};

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
fn dispatch(headers: &HeaderMap, json: Json<Payload>) -> Result<String> {
    fn inner(
        headers: &HeaderMap,
        json: Json<Payload>,
    ) -> std::result::Result<String, Box<dyn std::error::Error>> {
        let event_type = headers.get("X-Github-Event").ok_or("missing header")?;

        let event_type = event_type.to_str()?;

        match event_type {
            "ping" => tracing::info!("got ping"),
            "issue_comment" => {
                let issue: IssueCommentPayload = json.0.try_into()?;
                tracing::info!("got issue comment {:?}", issue.action);
            }
            "issues" => {
                tracing::info!("got issue {:?}", json.action);
            }
            _ => tracing::warn!("unknown event \"{event_type}\" (ignored)"),
        }
        Ok("OK".into())
    }

    inner(headers, json).map_err(|err| {
        tracing::warn!("error: {err}");
        Error::from_status(StatusCode::BAD_REQUEST)
    })
}
