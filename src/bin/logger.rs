use poem::{listener::TcpListener, Result, Route, Server};

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "poem=debug,logger=debug,info");
    }

    use tracing_subscriber::{fmt::format::FmtSpan, EnvFilter};
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env().add_directive("logger=info".parse().unwrap()),
        )
        .with_span_events(FmtSpan::FULL)
        .init();

    let ghwh = poem_github_webhook::GithubWebhook::new("foo");
    let app = Route::new().nest("/webhook", ghwh.route());

    Server::new(TcpListener::bind("0.0.0.0:9000"))
        .run(app)
        .await
}
