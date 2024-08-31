mod request;
mod response;

use std::sync::Arc;

use clap::Parser;
use rand::{Rng, SeedableRng};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;
use std::time::Instant;


/// Contains information parsed from the command-line invocation of balancebeam. The Clap macros
/// provide a fancy way to automatically construct a command-line argument parser.
#[derive(Parser, Debug)]
#[command(about = "Fun with load balancing")]
struct CmdOptions {
    /// "IP/port to bind to"
    #[arg(short, long, default_value = "0.0.0.0:1100")]
    bind: String,
    /// "Upstream host to forward requests to"
    #[arg(short, long)]
    upstream: Vec<String>,
    /// "Perform active health checks on this interval (in seconds)"
    #[arg(long, default_value = "10")]
    active_health_check_interval: usize,
    /// "Path to send request to for active health checks"
    #[arg(long, default_value = "/")]
    active_health_check_path: String,
    /// "Maximum number of requests to accept per IP per minute (0 = unlimited)"
    #[arg(long, default_value = "0")]
    max_requests_per_minute: usize,
}

/// Contains information about the state of balancebeam (e.g. what servers we are currently proxying
/// to, what servers have failed, rate limiting counts, etc.)
///
/// You should add fields to this struct in later milestones.
#[derive(Debug, Clone)]
struct ProxyState {
    /// How frequently we check whether upstream servers are alive (Milestone 4)
    #[allow(dead_code)]
    active_health_check_interval: usize,
    /// Where we should send requests when doing active health checks (Milestone 4)
    #[allow(dead_code)]
    active_health_check_path: String,
    /// Maximum number of requests an individual IP can make in a minute (Milestone 5)
    #[allow(dead_code)]
    max_requests_per_minute: usize,
    /// Addresses of servers that we are proxying to
    upstream_addresses: Vec<String>,
    /// previous time request
    /// previouse_request: usize,
    /// Right time request
    right_time_request: usize,
    /// Windows start time
    window_start_time: usize,
}

#[tokio::main]
async fn main() {
    // Initialize the logging library. You can print log messages using the `log` macros:
    // https://docs.rs/log/0.4.8/log/ You are welcome to continue using print! statements; this
    // just looks a little prettier.
    if let Err(_) = std::env::var("RUST_LOG") {
        std::env::set_var("RUST_LOG", "debug");
    }
    pretty_env_logger::init();

    // Parse the command line arguments passed to this program
    let options = CmdOptions::parse();
    if options.upstream.len() < 1 {
        log::error!("At least one upstream server must be specified using the --upstream option.");
        std::process::exit(1);
    }

    // Start listening for connections
    let listener = match TcpListener::bind(&options.bind).await {
        Ok(listener) => listener,
        Err(err) => {
            log::error!("Could not bind to {}: {}", options.bind, err);
            std::process::exit(1);
        }
    };
    log::info!("Listening for requests on {}", options.bind);

    // Handle incoming connections
    let state = ProxyState {
        upstream_addresses: options.upstream,
        active_health_check_interval: options.active_health_check_interval,
        active_health_check_path: options.active_health_check_path,
        max_requests_per_minute: options.max_requests_per_minute,
        // previouse_request: 0,
        right_time_request: 0,
        window_start_time: Instant::now().elapsed().as_secs() as usize,
    };

    let mut start_time = Instant::now(); // Get the start time
    let active_health_check_interval = state.active_health_check_interval;
    let upstreams = state.upstream_addresses.clone();
    let state = Arc::new(RwLock::new(state));
    loop {
        let duration = start_time.elapsed();
        if duration.as_secs() >= active_health_check_interval as u64 {
            health_check(&state, &upstreams).await;
            start_time = Instant::now();
        }

        let (stream, _) = listener.accept().await.unwrap();
        let state_clone = state.clone();
        tokio::spawn(async move {
            handle_connection(stream, &state_clone).await;
        });
    }
}

async fn health_check(state: &RwLock<ProxyState>, upstreams: &Vec<String>) {
    let path;
    {
        let state_read = state.read().await;
        path = state_read.active_health_check_path.clone();
    }

    for upstream in upstreams.iter() {
        log::info!("Performing active health check on {}", upstream);

        let request = http::Request::builder()
            .method(http::Method::GET)
            .uri(&path)
            .header("Host", upstream)
            .body(Vec::new())
            .unwrap();

        let mut upstream_stream = match TcpStream::connect(upstream).await {
            Ok(stream) => {stream},
            Err(err) => {
                {
                    let mut state_write = state.write().await;
                    state_write.upstream_addresses.retain(|addr| addr != upstream);
                    log::info!("Tcp connect failed, Drop Upstream addresses: {:?} for {}", upstream, err);
                }
                continue;
            }
        };
        if let Err(error) = request::write_to_stream(&request, &mut upstream_stream).await {
            log::warn!("Failed to write request to upstream: {}", error);
            continue;
        }else {
            log::info!("Successfully connected to upstream: {}", upstream);
            {
                if let Ok(response) = response::read_from_stream(&mut upstream_stream, &http::Method::GET).await {
                    if response.status().is_success() {
                        log::info!("Upstream {} returned 200 OK", upstream);
                        let mut state_write = state.write().await;
                        if !state_write.upstream_addresses.contains(&upstream.to_string()) {
                            state_write.upstream_addresses.push(upstream.to_string());
                            log::info!("Restored upstream: {}", upstream);
                            continue;
                        }
                    } else {
                        log::warn!("Upstream {} returned non-200 status: {}", upstream, response.status());
                        let mut state_write = state.write().await;
                        state_write.upstream_addresses.retain(|addr| addr != upstream);
                        log::info!("Removed upstream {} from active addresses due to non-200 response", upstream);
                        continue;
                    }
                }else {
                    let mut state_write = state.write().await;
                    state_write.upstream_addresses.retain(|addr| addr != upstream);
                    log::info!("Removed upstream {} from active addresses due to can't get response", upstream);
                    continue;
                }
            }
        }

    }
}
async fn connect_to_upstream(state: &RwLock<ProxyState>) -> Result<TcpStream, std::io::Error> {
    let mut rng = rand::rngs::StdRng::from_entropy();
    loop {
        let upstream_ip;
        {
            let state_read = state.read().await;
            log::info!("{:?}", state_read); // Printing the state for debugging

            if state_read.upstream_addresses.is_empty() {
                return Err(std::io::Error::new(std::io::ErrorKind::Other, "No upstream addresses available"));
            }

            let upstream_idx = rng.gen_range(0..state_read.upstream_addresses.len());
            upstream_ip = state_read.upstream_addresses[upstream_idx].clone();
        }
        // let mut state_write = state.write().await;
        let connection_result = TcpStream::connect(&upstream_ip).await;

        if let Err(err) = connection_result {
            log::warn!("Failed to connect to upstream {}: {}", upstream_ip, err);
            // If connection failed, write lock the state and remove the failed upstream IP
            let mut state_write = state.write().await;
            state_write.upstream_addresses.retain(|ip| ip != &upstream_ip);
            log::warn!("Removed failed upstream: {}", upstream_ip);
            log::info!("{:?}", state_write);
            // Return the original error
            continue;
        }else {
            return connection_result;
        }
    }
}

async fn send_response(client_conn: &mut TcpStream, response: &http::Response<Vec<u8>>) {
    let client_ip = client_conn.peer_addr().unwrap().ip().to_string();
    log::info!("{} <- {}", client_ip, response::format_response_line(&response));
    if let Err(error) = response::write_to_stream(&response, client_conn).await {
        log::warn!("Failed to send response to client: {}", error);
        return;
    }
}

async fn handle_connection(mut client_conn: TcpStream, state: &RwLock<ProxyState>) {
    let client_ip = client_conn.peer_addr().unwrap().ip().to_string();
    log::info!("Connection received from {}", client_ip);
    // Open a connection to a random destination server
    let mut upstream_conn = match connect_to_upstream(&state).await {
        Ok(stream) => stream,
        Err(_error) => {
            let response = response::make_http_error(http::StatusCode::BAD_GATEWAY);
            send_response(&mut client_conn, &response).await;
            return;
        }
    };
    let upstream_ip = client_conn.peer_addr().unwrap().ip().to_string();

    // The client may now send us one or more requests. Keep trying to read requests until the
    // client hangs up or we get an error.
    loop {
        // Read a request from the client
        let mut request = match request::read_from_stream(&mut client_conn).await {
            Ok(request) => {
                let is_rate_limit = rate_limit(&state).await;
                if is_rate_limit {
                    let response = response::make_http_error(http::StatusCode::TOO_MANY_REQUESTS);
                    send_response(&mut client_conn, &response).await;
                    continue;
                }else { // update the request count
                    let current_time = Instant::now().elapsed().as_secs() as usize;
                    let window_start_time;
                    {
                        window_start_time = state.read().await.window_start_time;
                    }
                    if current_time - window_start_time >= 60 {
                        if current_time - window_start_time >= 60 {
                            state.write().await.window_start_time = current_time;
                            state.write().await.right_time_request = 0;
                        }
                    }
                }
                state.write().await.right_time_request += 1;
                request
            },
            // Handle case where client closed connection and is no longer sending requests
            Err(request::Error::IncompleteRequest(0)) => {
                log::debug!("Client finished sending requests. Shutting down connection");
                return;
            }
            // Handle I/O error in reading from the client
            Err(request::Error::ConnectionError(io_err)) => {
                log::info!("Error reading request from client stream: {}", io_err);
                return;
            }
            Err(error) => {
                log::debug!("Error parsing request: {:?}", error);
                let response = response::make_http_error(match error {
                    request::Error::IncompleteRequest(_)
                    | request::Error::MalformedRequest(_)
                    | request::Error::InvalidContentLength
                    | request::Error::ContentLengthMismatch => http::StatusCode::BAD_REQUEST,
                    request::Error::RequestBodyTooLarge => http::StatusCode::PAYLOAD_TOO_LARGE,
                    request::Error::ConnectionError(_) => http::StatusCode::SERVICE_UNAVAILABLE,
                });
                send_response(&mut client_conn, &response).await;
                continue;
            }
        };
        log::info!(
            "{} -> {}: {}",
            client_ip,
            upstream_ip,
            request::format_request_line(&request)
        );

        // Add X-Forwarded-For header so that the upstream server knows the client's IP address.
        // (We're the ones connecting directly to the upstream server, so without this header, the
        // upstream server will only know our IP, not the client's.)
        request::extend_header_value(&mut request, "x-forwarded-for", &client_ip);

        // Forward the request to the server
        if let Err(error) = request::write_to_stream(&request, &mut upstream_conn).await {
            log::error!("Failed to send request to upstream {}: {}", upstream_ip, error);
            let response = response::make_http_error(http::StatusCode::BAD_GATEWAY);
            send_response(&mut client_conn, &response).await;
            return;
        }
        log::debug!("Forwarded request to server");

        // Read the server's response
        let response = match response::read_from_stream(&mut upstream_conn, request.method()).await {
            Ok(response) => response,
            Err(error) => {
                log::error!("Error reading response from server: {:?}", error);
                let response = response::make_http_error(http::StatusCode::BAD_GATEWAY);
                send_response(&mut client_conn, &response).await;
                return;
            }
        };
        // Forward the response to the client
        send_response(&mut client_conn, &response).await;
        log::debug!("Forwarded response to client");
    }
}

async fn rate_limit(state: &RwLock<ProxyState>) -> bool{
    let state_read = state.read().await;
    if state_read.max_requests_per_minute == 0 {
        return false;
    }
    let request_count = state_read.right_time_request;
    let max_requests = state_read.max_requests_per_minute;
    if request_count >= max_requests {
        log::warn!("Rate limit exceeded: {} requests per second", request_count);
        return true;
    }
    false
}