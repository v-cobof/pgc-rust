use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, StatusCode, Server};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::env;
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Write;

// Estrutura para o resultado RLE (Deve corresponder ao processador)
#[derive(Debug, Serialize, Deserialize, Clone)]
struct RLEPayload {
    rle_data: Vec<RLEOutput>,
    compressed_string: String,
    original_count: usize,
    compressed_count: usize,
    reduction: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct RLEOutput {
    category_name: String,
    quantity: u32,
}

async fn handle_request(
    req: Request<Body>,
    log_file: String,
) -> Result<Response<Body>, anyhow::Error> {
    match (req.method(), req.uri().path()) {
        (&Method::GET, "/") => Ok(Response::new(Body::from(
            "Receiver: Envie dados RLE via POST para /receber",
        ))),

        (&Method::POST, "/receber") => {
            println!("📥 Receiver: Recebendo pacotes RLE...");
            
            let byte_stream = match hyper::body::to_bytes(req).await {
                Ok(bytes) => bytes,
                Err(e) => {
                    eprintln!("Erro ao ler corpo da requisição: {}", e);
                    let mut res = response_build(&serde_json::json!({"error": "Erro ao ler corpo"}).to_string());
                    *res.status_mut() = StatusCode::BAD_REQUEST;
                    return Ok(res);
                }
            };

            let payload: RLEPayload = match serde_json::from_slice(&byte_stream) {
                Ok(data) => data,
                Err(e) => {
                    eprintln!("Erro de parsing JSON: {}", e);
                    let mut res = response_build(&serde_json::json!({"error": "JSON RLE inválido"}).to_string());
                    *res.status_mut() = StatusCode::BAD_REQUEST;
                    return Ok(res);
                }
            };

            println!("💾 Salvando string comprimida: {}", payload.compressed_string);
            println!("📊 Redução: {}", payload.reduction);

            // Salvando no arquivo (WASM Precisa de --dir habilitado no runtime)
            let mut file = match OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_file) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("❌ Erro ao abrir arquivo: {}", e);
                    let mut res = response_build(&serde_json::json!({"error": format!("Erro de E/S: {}", e)}).to_string());
                    *res.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                    return Ok(res);
                }
            };

            let timestamp = chrono::Utc::now().to_rfc3339();
            let entry = format!("[{}] {} | Redução: {}\n", 
                                timestamp, payload.compressed_string, payload.reduction);
            
            if let Err(e) = file.write_all(entry.as_bytes()) {
                eprintln!("❌ Erro ao escrever no arquivo: {}", e);
                let mut res = response_build(&serde_json::json!({"error": "Erro ao gravar"}).to_string());
                *res.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                return Ok(res);
            }

            let mut res = response_build(&serde_json::json!({"status": "recebido_e_salvo"}).to_string());
            *res.status_mut() = StatusCode::OK;
            Ok(res)
        },

        (&Method::OPTIONS, _) => Ok(response_build("")),

        _ => {
            let mut not_found = Response::default();
            *not_found.status_mut() = StatusCode::NOT_FOUND;
            Ok(not_found)
        }
    }
}

fn response_build(body: &str) -> Response<Body> {
    Response::builder()
        .header("Access-Control-Allow-Origin", "*")
        .header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
        .header("Access-Control-Allow-Headers", "api,Keep-Alive,User-Agent,Content-Type")
        .header("Content-Type", "application/json")
        .body(Body::from(body.to_owned()))
        .unwrap()
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args: Vec<String> = env::args().collect();
    
    // Porta: argumento 1 ou env PORTA ou 8082
    let porta_str = args.get(1)
        .cloned()
        .unwrap_or_else(|| env::var("PORTA").unwrap_or_else(|_| "8082".to_string()));
    let porta: u16 = porta_str.parse().expect("Porta inválida");

    // Arquivo de Log: argumento 2 ou env LOG_FILE ou default
    let log_file = args.get(2)
        .cloned()
        .unwrap_or_else(|| env::var("LOG_FILE").unwrap_or_else(|_| "monitoramento.txt".to_string()));

    // Ambiente: argumento 3 ou env AMBIENTE ou Cloud
    let ambiente = args.get(3)
        .cloned()
        .unwrap_or_else(|| env::var("AMBIENTE").unwrap_or_else(|_| "Cloud".to_string()));

    let addr = SocketAddr::from(([0, 0, 0, 0], porta));
    
    let log_file_clone = log_file.clone();
    let make_svc = make_service_fn(move |_| {
        let log_file = log_file_clone.clone();
        async move {
            Ok::<_, Infallible>(service_fn(move |req| {
                let log_file = log_file.clone();
                handle_request(req, log_file)
            }))
        }
    });

    println!("==========================================");
    println!("🏛️ Receiver (Cloud) rodando na porta {}", porta);
    println!("🌍 Ambiente: {}", ambiente);
    println!("📝 Arquivo de log: {}", log_file);
    println!("==========================================");

    let server = Server::bind(&addr).serve(make_svc);

    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }
   
    Ok(())
}
