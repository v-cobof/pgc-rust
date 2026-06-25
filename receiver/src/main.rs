use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, StatusCode, Server};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::env;
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Write;

// ==============================================================================
// ESTRUTURAS DE DADOS
// ==============================================================================

// Estrutura para receber o payload de compressão RLE do processador/névoa.
// Deve espelhar fielmente a estrutura definida no processador.
#[derive(Debug, Serialize, Deserialize, Clone)]
struct RLEPayload {
    rle_data: Vec<RLEOutput>,
    compressed_string: String,
    original_count: usize,
    compressed_count: usize,
    reduction: String,
}

// Representação de uma categoria e sua respectiva repetição consecutiva (corrida).
#[derive(Debug, Serialize, Deserialize, Clone)]
struct RLEOutput {
    category_name: String,
    quantity: u32,
}

// ==============================================================================
// HANDLER DE REQUISIÇÕES (ROUTING)
// ==============================================================================
// Recebe requisições HTTP da rede e executa a lógica apropriada.
async fn handle_request(
    req: Request<Body>,
    log_file: String,
) -> Result<Response<Body>, anyhow::Error> {
    match (req.method(), req.uri().path()) {
        // Rota GET /: Apenas exibe instruções de uso básicas
        (&Method::GET, "/") => Ok(Response::new(Body::from(
            "Receiver: Envie dados RLE via POST para /receber",
        ))),

        // Rota POST /receber: Recebe a carga comprimida RLE, decodifica e grava em disco
        (&Method::POST, "/receber") => {
            println!("📥 Receiver: Recebendo pacotes RLE...");
            
            // Lê de forma assíncrona os bytes do corpo da requisição HTTP
            let byte_stream = match hyper::body::to_bytes(req).await {
                Ok(bytes) => bytes,
                Err(e) => {
                    eprintln!("Erro ao ler corpo da requisição: {}", e);
                    let mut res = response_build(&serde_json::json!({"error": "Erro ao ler corpo"}).to_string());
                    *res.status_mut() = StatusCode::BAD_REQUEST;
                    return Ok(res);
                }
            };

            // Tenta deserializar o JSON recebido na estrutura RLEPayload
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

            // Abre o arquivo de log para adição contínua (append mode).
            // A execução em ambientes WebAssembly (WASI) exige que o runtime (como WasmEdge)
            // possua permissões de diretório --dir habilitadas apontando para o diretório atual.
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

            // Escreve a entrada no arquivo com timestamp em formato RFC3339
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

        // Rota OPTIONS para suporte a CORS (pré-requisições dos navegadores)
        (&Method::OPTIONS, _) => Ok(response_build("")),

        // Qualquer outro endpoint retorna HTTP 404 Not Found
        _ => {
            let mut not_found = Response::default();
            *not_found.status_mut() = StatusCode::NOT_FOUND;
            Ok(not_found)
        }
    }
}

// ==============================================================================
// FUNÇÃO AUXILIAR DE RESPOSTA HTTP (CORS + HEADERS)
// ==============================================================================
// Constrói a resposta Hyper configurando os cabeçalhos para evitar bloqueios de CORS.
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
// Inicializa o servidor HTTP Hyper para escutar conexões e registrar logs na Nuvem.
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args: Vec<String> = env::args().collect();
    
    // Porta do servidor: Argumento 1, env var PORTA, ou 8082 default
    let porta_str = args.get(1)
        .cloned()
        .unwrap_or_else(|| env::var("PORTA").unwrap_or_else(|_| "8082".to_string()));
    let porta: u16 = porta_str.parse().expect("Porta inválida");

    // Nome do arquivo onde serão gravados os payloads recebidos: Argumento 2, env var, ou monitoramento.txt
    let log_file = args.get(2)
        .cloned()
        .unwrap_or_else(|| env::var("LOG_FILE").unwrap_or_else(|_| "monitoramento.txt".to_string()));

    // Nome do ambiente de execução (Cloud)
    let ambiente = args.get(3)
        .cloned()
        .unwrap_or_else(|| env::var("AMBIENTE").unwrap_or_else(|_| "Cloud".to_string()));

    // Define o endereço IP do socket servidor (escuta em todas as interfaces)
    let addr = SocketAddr::from(([0, 0, 0, 0], porta));
    
    // Clone para mover as referências de forma segura para dentro do escopo do Hyper
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

    // Inicializa o binding e executa o servidor assíncrono
    let server = Server::bind(&addr).serve(make_svc);

    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }
   
    Ok(())
}
