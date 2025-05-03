use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::Host;
use axum::handler::HandlerWithoutStateExt;
use axum::response::Redirect;
use axum::BoxError;
use axum_server::tls_rustls::RustlsConfig;
use http::StatusCode;
use http::Uri;
use serde::Deserialize;
use serde::Serialize;
use tower_http::cors::Any;
use tower_http::cors::CorsLayer;
use tower_http::trace::DefaultMakeSpan;
use tower_http::trace::TraceLayer;

use axum::extract::DefaultBodyLimit;
use axum::http::Method;

use crate::errors::ServerError;
use crate::router;

use shared::config::Ports;
use shared::config::KEY_CHAIN_PATH;

#[derive(Debug, Serialize, Deserialize)]
pub struct EmbeddedHttpServer;

impl EmbeddedHttpServer {
    pub async fn run(socket_address: Arc<IpAddr>) -> Result<(), ServerError> {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .init();

        let cors_layer = CorsLayer::new()
            .allow_headers(Any) //TODO: add key gen for security and middleware
            .allow_methods([Method::GET, Method::POST])
            .allow_origin(Any); //TODO: restrict to IP address

        //TODO: improve data limit it is currently set to define file limit layer as 10GB
        // see information here <https://docs.rs/axum/0.6.2/axum/extract/struct.DefaultBodyLimit.html#%E2%80%A6>
        let file_size_limit = 10 * 1024 * 1024 * 1024;
        let file_limit = DefaultBodyLimit::max(file_size_limit);

        // optional: spawn a second server to redirect http requests to this server
        let ports = Ports::default();
        tokio::spawn(Self::redirect_http_to_https(
            ports,
            Arc::clone(&socket_address),
        ));

        // configure certificate and private key used by https
        let config = RustlsConfig::from_pem_file(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join(KEY_CHAIN_PATH)
                .join("cert.pem"),
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join(KEY_CHAIN_PATH)
                .join("key.pem"),
        )
        .await
        .map_err(|err| ServerError::StartUpError(err.to_string()))?;

        //  run the https server on localhost then feed off the connection using the wifi gateway, the same way Vite/Vue CLI would do the core server
        // this is currently achieved by binding the server to the device default ip address

        let socket_address = format!("{:?}:{:?}", socket_address, ports.https);
        let socket_address = socket_address
            .parse::<std::net::SocketAddr>()
            .map_err(|err| ServerError::StartUpError(err.to_string()))?;

        println!("my local ip is {}", socket_address);
        let app = router::app()
            .layer(file_limit)
            .layer(cors_layer)
            .layer(tower_http::trace::TraceLayer::new_for_http())
            .layer(
                TraceLayer::new_for_http()
                    .make_span_with(DefaultMakeSpan::default().include_headers(true)),
            );

        tracing::debug!(" the server port is http://{}", socket_address);
        axum_server::bind_rustls(socket_address, config)
            .serve(app.into_make_service())
            .await
            .map_err(|err| ServerError::StartUpError(err.to_string()))?;

        Ok(())
    }

    async fn redirect_http_to_https(
        ports: Ports,
        socket_address: Arc<IpAddr>,
    ) -> Result<(), ServerError> {
        fn make_https(host: String, uri: Uri, ports: Ports) -> Result<Uri, BoxError> {
            let mut parts = uri.into_parts();

            parts.scheme = Some(axum::http::uri::Scheme::HTTPS);

            if parts.path_and_query.is_none() {
                parts.path_and_query = Some("/".parse().unwrap());
            }

            let https_host = host.replace(&ports.http.to_string(), &ports.https.to_string());
            parts.authority = Some(https_host.parse()?);

            Ok(Uri::from_parts(parts)?)
        }

        let redirect = move |Host(host): Host, uri: Uri| async move {
            match make_https(host, uri, ports) {
                Ok(uri) => Ok(Redirect::permanent(&uri.to_string())),
                Err(error) => {
                    tracing::warn!(%error, "failed to convert URI to HTTPS");
                    Err(StatusCode::BAD_REQUEST)
                }
            }
        };

        let socket_address = format!("{:?}:{:?}", socket_address, ports.http);
        let socket_address = socket_address
            .parse::<std::net::SocketAddr>()
            .map_err(|err| ServerError::StartUpError(err.to_string()))?;

        let listener = tokio::net::TcpListener::bind(socket_address).await.unwrap();
        tracing::debug!("listening on {}", listener.local_addr().unwrap());
        axum::serve(listener, redirect.into_make_service())
            .await
            .unwrap();

        Ok(())
    }
}
