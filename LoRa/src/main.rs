use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, StatusCode, Server};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::env;
use serde::{Deserialize, Serialize};
use reqwest::Client;

// ==============================================================================
// ESTRUTURAS DE DADOS
// ==============================================================================

// Estrutura para os dados brutos recebidos dos sensores.
// Deve ser mantida idêntica à do processador e simulador.
#[derive(Debug, Deserialize, Serialize, Clone)]
struct SensorData {
    temperature: f32,
    humidity: f32,
    timestamp: u64,
}

// Estrutura para os payloads de dados compactados RLE.
#[derive(Debug, Serialize, Deserialize, Clone)]
struct RLEPayload {
    rle_data: Vec<RLEOutput>,
    compressed_string: String,
    original_count: usize,
    compressed_count: usize,
    reduction: String,
}

// Representação de categorias comprimidas individuais.
#[derive(Debug, Serialize, Deserialize, Clone)]
struct RLEOutput {
    category_name: String,
    quantity: u32,
}

// ==============================================================================
// HANDLER DE REQUISIÇÕES (LÓGICA DE PROXY/RETRAMISSÃO)
// ==============================================================================
// O Gateway LoRa atua como uma ponte de comunicação de rede,
// recebendo e retransmitindo pacotes.
async fn handle_request(
    req: Request<Body>,
    client: Client,
    processor_url: String,
) -> Result<Response<Body>, anyhow::Error> {
    match (req.method(), req.uri().path()) {
        // Rota GET /: Fornece informações sobre as rotas disponíveis
        (&Method::GET, "/") => Ok(Response::new(Body::from(
            "LoRa Relay: Envie dados via /inserir (Normal) ou /receber (RLE)",
        ))),

        // Rota POST /receber: Proxy para retransmitir payloads compactados RLE
        (&Method::POST, "/receber") => {
            println!("📥 LoRa: Recebendo pacote RLE (Comprimido)...");
            let byte_stream = match hyper::body::to_bytes(req).await {
                Ok(bytes) => bytes,
                Err(e) => {
                    eprintln!("Erro ao ler corpo da requisição: {}", e);
                    let mut res = response_build(&serde_json::json!({"error": "Erro ao ler corpo"}).to_string());
                    *res.status_mut() = StatusCode::BAD_REQUEST;
                    return Ok(res);
                }
            };

            let rle_payload: RLEPayload = match serde_json::from_slice(&byte_stream) {
                Ok(data) => data,
                Err(e) => {
                    eprintln!("Erro de parsing JSON RLE: {}", e);
                    let mut res = response_build(&serde_json::json!({"error": "JSON RLE inválido"}).to_string());
                    *res.status_mut() = StatusCode::BAD_REQUEST;
                    return Ok(res);
                }
            };

            println!("➡️ LoRa: Retransmitindo pacote RLE para o alvo... (Redução: {})", rle_payload.reduction);

            // Dispara requisição POST encaminhando o payload JSON para o processador alvo
            let response = client.post(&processor_url)
                .json(&rle_payload)
                .send()
                .await;

            match response {
                Ok(res) => {
                    let status = res.status();
                    println!("✅ Alvo respondeu com status: {}", status);
                    let mut relay_res = response_build(&serde_json::json!({"status": "rle_retransmitido", "target_status": status.as_u16()}).to_string());
                    *relay_res.status_mut() = if status.is_success() { StatusCode::OK } else { StatusCode::BAD_GATEWAY };
                    Ok(relay_res)
                }
                Err(e) => {
                    eprintln!("❌ Erro ao conectar com o alvo: {}", e);
                    let mut res = response_build(&serde_json::json!({"error": "Falha na retransmissão RLE"}).to_string());
                    *res.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                    Ok(res)
                }
            }
        },

        // Rota POST /inserir: Proxy para retransmitir dados normais de sensoriamento contínuo
        (&Method::POST, "/inserir") => {
            println!("📡 LoRa: Recebendo dados do sensor...");
            
            let byte_stream = match hyper::body::to_bytes(req).await {
                Ok(bytes) => bytes,
                Err(e) => {
                    eprintln!("Erro ao ler corpo da requisição: {}", e);
                    let mut res = response_build(&serde_json::json!({"error": "Erro ao ler corpo"}).to_string());
                    *res.status_mut() = StatusCode::BAD_REQUEST;
                    return Ok(res);
                }
            };

            let sensor_data: SensorData = match serde_json::from_slice(&byte_stream) {
                Ok(data) => data,
                Err(e) => {
                    eprintln!("Erro de parsing JSON: {}", e);
                    let mut res = response_build(&serde_json::json!({"error": "JSON inválido"}).to_string());
                    *res.status_mut() = StatusCode::BAD_REQUEST;
                    return Ok(res);
                }
            };

            println!("➡️ Retransmitindo para o processador: {:.1}°C | {:.1}%", 
                     sensor_data.temperature, sensor_data.humidity);

            // Retransmite usando o cliente http assíncrono para o processador alvo
            let response = client.post(&processor_url)
                .json(&sensor_data)
                .send()
                .await;

            match response {
                Ok(res) => {
                    let status = res.status();
                    println!("✅ Processador respondeu com status: {}", status);
                    
                    let mut relay_res = response_build(&serde_json::json!({
                        "status": "retransmitido",
                        "processor_status": status.as_u16()
                    }).to_string());
                    *relay_res.status_mut() = if status.is_success() { StatusCode::OK } else { StatusCode::BAD_GATEWAY };
                    Ok(relay_res)
                }
                Err(e) => {
                    eprintln!("❌ Erro ao conectar com o processador: {}", e);
                    let mut res = response_build(&serde_json::json!({"error": "Falha na retransmissão"}).to_string());
                    *res.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                    Ok(res)
                }
            }
        },

        // Rota OPTIONS para suporte a solicitações CORS pré-vôo
        (&Method::OPTIONS, _) => Ok(response_build("")),

        // 404 para outras URLs
        _ => {
            let mut not_found = Response::default();
            *not_found.status_mut() = StatusCode::NOT_FOUND;
            Ok(not_found)
        }
    }
}

// Constrói a resposta Hyper configurando CORS headers
fn response_build(body: &str) -> Response<Body> {
    Response::builder()
        .header("Access-Control-Allow-Origin", "*")
        .header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
        .header("Access-Control-Allow-Headers", "api,Keep-Alive,User-Agent,Content-Type")
        .header("Content-Type", "application/json")
        .body(Body::from(body.to_owned()))
        .unwrap()
}

// ==============================================================================
// FUNÇÃO PRINCIPAL (ENTRYPOINT)
// ==============================================================================
// Inicializa o servidor HTTP do relay LoRa.
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args: Vec<String> = env::args().collect();
    
    // Porta de escuta: Argumento 1, env var PORTA, ou 8080 default
    let porta_str = args.get(1)
        .cloned()
        .unwrap_or_else(|| env::var("PORTA").unwrap_or_else(|_| "8080".to_string()));
    let porta: u16 = porta_str.parse().expect("Porta inválida");

    // URL do Processador/Névoa alvo: Argumento 2, env var PROCESSOR_URL, ou localhost default
    let processor_url = args.get(2)
        .cloned()
        .unwrap_or_else(|| env::var("PROCESSOR_URL").unwrap_or_else(|_| "http://localhost:8081/inserir".to_string()));

    // Nome do ambiente de execução (Fog/Relay)
    let ambiente = args.get(3)
        .cloned()
        .unwrap_or_else(|| env::var("AMBIENTE").unwrap_or_else(|_| "Fog".to_string()));

    let addr = SocketAddr::from(([0, 0, 0, 0], porta));
    
    // Cliente HTTP reusável para o encaminhamento de pacotes
    let client = Client::new();
    
    let processor_url_clone = processor_url.clone();
    let make_svc = make_service_fn(move |_| {
        let client = client.clone();
        let processor_url = processor_url_clone.clone();
        async move {
            let processor_url = processor_url.clone();
            Ok::<_, Infallible>(service_fn(move |req| {
                let client = client.clone();
                let processor_url = processor_url.clone();
                handle_request(req, client, processor_url)
            }))
        }
    });

    println!("==========================================");
    println!("📡 Relay LoRa rodando na porta {}", porta);
    println!("🌍 Ambiente: {}", ambiente);
    println!("➡️  Alvo: {}", processor_url);
    println!("==========================================");

    let server = Server::bind(&addr).serve(make_svc);

    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }
   
    Ok(())
}
