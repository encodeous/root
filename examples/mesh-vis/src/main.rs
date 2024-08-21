use crate::graph_parse::{load, save};
use crate::sim::tick_state;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Empty, Full};
use hyper::body::Bytes;
use hyper::http::HeaderValue;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_staticfile::Static;
use hyper_util::rt::TokioIo;
use mime_guess::Mime;
use root::concepts::packet::Packet;
use root::framework::{MACSignature, RoutingSystem};
use root::router::{Router, INF, DummyMAC, NoMACSystem};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use std::fs::File;
use std::io;
use std::io::{BufRead, Error};
use std::net::SocketAddr;
use std::path::Path;
use std::str::FromStr;
use std::sync::Mutex;
use tokio::net::TcpListener;
use yaml_rust2::yaml::Hash;
use yaml_rust2::{Yaml, YamlEmitter, YamlLoader};
use log::{error, info, warn};
use simplelog::*;

mod graph_parse;
mod sim;
mod vis;

pub struct GraphSystem {
    router: Router<Self>,
}

impl Clone for GraphSystem {
    fn clone(&self) -> Self {
        todo!() // don't actually need to clone, rust's type system is too strict lol
    }
}

impl RoutingSystem for GraphSystem {
    type NodeAddress = u8;
    type Link = u8;
    type MACSystem = NoMACSystem;
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    CombinedLogger::init(
        vec![
            TermLogger::new(LevelFilter::Debug, Config::default(), TerminalMode::Mixed, ColorChoice::Auto)
        ]
    ).unwrap();

    if !Path::new("./public").exists(){
        error!("Unable to find the ./public folder. Make sure to run the program in the correct directory.");
        return Ok(());
    }


    let addr = SocketAddr::from(([0, 0, 0, 0], 9999));

    let listener = TcpListener::bind(addr).await?;

    // We start a loop to continuously accept incoming connections
    loop {
        let (stream, _) = listener.accept().await?;

        // Use an adapter to access something implementing `tokio::io` traits as if they implement
        // `hyper::rt` IO traits.
        let io = TokioIo::new(stream);

        // Spawn a tokio task to serve multiple connections concurrently
        tokio::task::spawn(async move {
            let static_ = Static::new(Path::new("public/"));
            // Finally, we bind the incoming connection to our `hello` service
            if let Err(err) = http1::Builder::new()
                // `service_fn` converts our function in a `Service`
                .serve_connection(io, service_fn(|req| sim_route(req, &static_)))
                .await
            {
                eprintln!("Error serving connection: {:?}", err);
            }
        });
    }
}

async fn sim_route(
    req: Request<hyper::body::Incoming>,
    static_: &Static,
) -> Result<Response<BoxBody<Bytes, anyhow::Error>>, anyhow::Error> {
    let path_str = req.uri().path().to_string();
    match (req.method(), path_str.as_str()) {
        (&Method::POST, "/sim_route") => {
            let body = req.collect().await?.to_bytes();
            let str = String::from_utf8(body.to_vec()).unwrap();

            let yaml = YamlLoader::load_from_str(str.as_str());

            if let Err(err) = yaml {
                let mut r_err = Response::new(full("Invalid YAML"));
                *r_err.status_mut() = StatusCode::BAD_REQUEST;
                return Ok(r_err);
            }

            let mut state = load(&yaml.unwrap()[0]);

            if let Err(err) = state {
                let mut r_err = Response::new(full(err.to_string()));
                *r_err.status_mut() = StatusCode::BAD_REQUEST;
                return Ok(r_err);
            }

            let gs = &mut state.unwrap();

            tick_state(gs);
            let new_state = save(gs);

            Ok(Response::new(full(yaml_to_str(&new_state))))
        }
        (&Method::GET, path) => {
            let mut resp = Response::new(full(
                static_
                    .clone()
                    .serve(req)
                    .await?
                    .collect()
                    .await?
                    .to_bytes(),
            ));
            let mime_type = mime_guess::from_path(path)
                .first_or(Mime::from_str("text/html").unwrap())
                .to_string();

            resp.headers_mut()
                .insert("Content-Type", HeaderValue::from_str(mime_type.as_str())?);
            Ok(resp)
        }
        // Return 404 Not Found for other routes.
        _ => {
            let mut not_found = Response::new(empty());
            *not_found.status_mut() = StatusCode::NOT_FOUND;
            Ok(not_found)
        }
    }
}

fn full<T: Into<Bytes>>(chunk: T) -> BoxBody<Bytes, anyhow::Error> {
    Full::new(chunk.into())
        .map_err(|never| match never {})
        .boxed()
}
fn empty() -> BoxBody<Bytes, anyhow::Error> {
    Empty::<Bytes>::new()
        .map_err(|never| match never {})
        .boxed()
}

fn yaml_to_str(yaml: &Yaml) -> String {
    let mut out = String::new();
    let mut yml = YamlEmitter::new(&mut out);
    yml.compact(true);
    yml.dump(yaml);
    out
}
