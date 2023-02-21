use poem::{
    async_trait,
    error::{BadRequest, InternalServerError},
    get, handler,
    http::StatusCode,
    listener::TcpListener,
    post, Endpoint, EndpointExt, IntoResponse, Middleware, Request, Response, Result, Route,
    Server,
};

struct GithubWebhook {
    secret: String,
}

impl GithubWebhook {
    pub fn new() -> Self {
        println!("GithubWebhook::new()");
        GithubWebhook {
            secret: "foo".into(),
        }
    }
}

impl<E: Endpoint> Middleware<E> for GithubWebhook {
    type Output = GithubWebhookImpl<E>;

    fn transform(&self, ep: E) -> Self::Output {
        GithubWebhookImpl(ep, self.secret.clone())
    }
}

struct GithubWebhookImpl<E>(E, String);

#[async_trait]
impl<E: Endpoint> Endpoint for GithubWebhookImpl<E> {
    type Output = Response;

    async fn call(&self, mut req: Request) -> Result<Self::Output> {
        println!("request: {}", req.uri().path());
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
                match res {
                    Ok(resp) => {
                        let resp = resp.into_response();
                        println!("response: {}", resp.status());
                        Ok(resp)
                    }
                    Err(err) => {
                        println!("error: {err}");
                        Err(err)
                    }
                }
            }
        } else {
            tracing::warn!("Event not signed but webhook secret configured, ignoring event");
            return Ok(Response::builder().status(StatusCode::BAD_REQUEST).finish());
        }
    }
}

#[handler]
fn handle_verified() -> String {
    "hello".to_string()
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "poem=debug");
    }
    tracing_subscriber::fmt::init();

    let app = Route::new()
        .at("/webhook", post(handle_verified))
        .with(GithubWebhook::new());

    Server::new(TcpListener::bind("0.0.0.0:9000"))
        .run(app)
        .await
}
