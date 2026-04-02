use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, StatusCode, Server};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::collections::HashMap;
use std::env;
use std::sync::{Arc, Mutex, RwLock};
use serde::{Deserialize, Serialize};
use chrono::{Local, Utc};

// Estrutura para dados recebidos dos sensores
#[derive(Debug, Deserialize, Serialize, Clone)]
struct SensorData {
    temperature: f32,
    humidity: f32,
    timestamp: u64,
}

// Estrutura para dados classificados
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ClassifiedData {
    category: u32,
    category_name: String,
    temperature: f32,
    humidity: f32,
    timestamp: u64,
}

// Dataset de treinamento (baseado na Tabela 1 do artigo)
#[derive(Debug, Clone)]
struct TrainingSample {
    category: u32,
    category_name: String,
    temp_min: f32,
    temp_max: f32,
    hum_min: f32,
    hum_max: f32,
}

// Estrutura para o resultado RLE
#[derive(Debug, Serialize, Deserialize, Clone)]
struct RLEOutput {
    category_name: String,
    quantity: u32,
}

// Estrutura para envio ao Receiver (Cloud)
#[derive(Debug, Serialize, Deserialize, Clone)]
struct RLEPayload {
    rle_data: Vec<RLEOutput>,
    compressed_string: String,
    original_count: usize,
    compressed_count: usize,
    reduction: String,
}

// Estado compartilhado da aplicação
struct AppState {
    sensor_data: Mutex<Vec<SensorData>>,
    classified_data: Mutex<Vec<ClassifiedData>>,
    rle_results: Mutex<Vec<RLEOutput>>,
    training_dataset: Vec<TrainingSample>,
}

impl AppState {
    fn new() -> Self {
        Self {
            sensor_data: Mutex::new(Vec::new()),
            classified_data: Mutex::new(Vec::new()),
            rle_results: Mutex::new(Vec::new()),
            training_dataset: Self::create_training_dataset(),
        }
    }
    
    // Dataset de treinamento baseado no artigo
    fn create_training_dataset() -> Vec<TrainingSample> {
        vec![
            TrainingSample { category: 0, category_name: "Critical dry".to_string(), temp_min: 0.0, temp_max: 4.0, hum_min: 0.0, hum_max: 35.0 },
            TrainingSample { category: 1, category_name: "Lower fail".to_string(), temp_min: 0.0, temp_max: 11.0, hum_min: 40.0, hum_max: 65.0 },
            TrainingSample { category: 2, category_name: "Marginal".to_string(), temp_min: 12.0, temp_max: 28.0, hum_min: 40.0, hum_max: 55.0 },
            TrainingSample { category: 3, category_name: "Upper Fail".to_string(), temp_min: 29.0, temp_max: 47.0, hum_min: 40.0, hum_max: 55.0 },
            TrainingSample { category: 4, category_name: "Cold and Humid".to_string(), temp_min: 0.0, temp_max: 11.0, hum_min: 70.0, hum_max: 100.0 },
            TrainingSample { category: 5, category_name: "Lower optimal".to_string(), temp_min: 12.0, temp_max: 14.0, hum_min: 60.0, hum_max: 100.0 },
            TrainingSample { category: 6, category_name: "Optimal".to_string(), temp_min: 15.0, temp_max: 17.0, hum_min: 60.0, hum_max: 100.0 },
            TrainingSample { category: 7, category_name: "Upper Optimal".to_string(), temp_min: 18.0, temp_max: 27.0, hum_min: 60.0, hum_max: 100.0 },
            TrainingSample { category: 8, category_name: "Upper Marginal".to_string(), temp_min: 28.0, temp_max: 31.0, hum_min: 60.0, hum_max: 100.0 },
            TrainingSample { category: 9, category_name: "Upper Fail (high hum)".to_string(), temp_min: 32.0, temp_max: 35.0, hum_min: 60.0, hum_max: 100.0 },
            TrainingSample { category: 10, category_name: "Critical".to_string(), temp_min: 36.0, temp_max: 47.0, hum_min: 60.0, hum_max: 100.0 },
        ]
    }
    
    // Normalizar dados (0-1) baseado nos ranges do dataset de treinamento
    fn normalize_value(&self, value: f32, min: f32, max: f32) -> f32 {
        (value - min) / (max - min)
    }
    
    // Calcular distância euclidiana entre dois pontos normalizados
    fn euclidean_distance(&self, t1: f32, h1: f32, t2: f32, h2: f32) -> f32 {
        ((t1 - t2).powi(2) + (h1 - h2).powi(2)).sqrt()
    }
    
    // Implementação do kNN (k=3 como no artigo)
    fn knn_classify(&self, temperature: f32, humidity: f32, k: usize) -> (u32, String) {
        // Encontrar ranges globais para normalização
        let all_temps: Vec<f32> = self.training_dataset.iter().flat_map(|t| vec![t.temp_min, t.temp_max]).collect();
        let all_hums: Vec<f32> = self.training_dataset.iter().flat_map(|t| vec![t.hum_min, t.hum_max]).collect();
        let temp_min = all_temps.iter().fold(f32::INFINITY, |a, &b| a.min(b));
        let temp_max = all_temps.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
        let hum_min = all_hums.iter().fold(f32::INFINITY, |a, &b| a.min(b));
        let hum_max = all_hums.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
        
        // Normalizar o ponto a ser classificado
        let norm_temp = self.normalize_value(temperature, temp_min, temp_max);
        let norm_hum = self.normalize_value(humidity, hum_min, hum_max);
        
        // Calcular distâncias para todos os pontos de treinamento (usando pontos médios das faixas)
        let mut distances: Vec<(f32, u32, String)> = self.training_dataset.iter().map(|sample| {
            let sample_temp_mid = (sample.temp_min + sample.temp_max) / 2.0;
            let sample_hum_mid = (sample.hum_min + sample.hum_max) / 2.0;
            let norm_sample_temp = self.normalize_value(sample_temp_mid, temp_min, temp_max);
            let norm_sample_hum = self.normalize_value(sample_hum_mid, hum_min, hum_max);
            
            let dist = self.euclidean_distance(norm_temp, norm_hum, norm_sample_temp, norm_sample_hum);
            (dist, sample.category, sample.category_name.clone())
        }).collect();
        
        // Ordenar por distância
        distances.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        
        // Pegar os k vizinhos mais próximos
        let neighbors = &distances[0..k.min(distances.len())];
        
        // Contar votos por categoria
        let mut votes: HashMap<u32, (u32, String)> = HashMap::new();
        for &(_, cat, ref name) in neighbors {
            let entry = votes.entry(cat).or_insert((0, name.clone()));
            entry.0 += 1;
        }
        
        // Encontrar categoria mais votada
        let (category, (_, name)) = votes.into_iter()
            .max_by_key(|&(_, (count, _))| count)
            .unwrap_or((0, (1, "Unknown".to_string())));
        
        (category, name)
    }
    
    // Implementação do RLE (Run-Length Encoding)
    fn apply_rle(&self, data: &[ClassifiedData]) -> Vec<RLEOutput> {
        if data.is_empty() {
            return Vec::new();
        }
        
        let mut result = Vec::new();
        let mut current_category = &data[0].category_name;
        let mut current_count = 1;
        
        for item in &data[1..] {
            if &item.category_name == current_category {
                current_count += 1;
            } else {
                result.push(RLEOutput {
                    category_name: current_category.clone(),
                    quantity: current_count,
                });
                current_category = &item.category_name;
                current_count = 1;
            }
        }
        
        // Adicionar o último grupo
        result.push(RLEOutput {
            category_name: current_category.clone(),
            quantity: current_count,
        });
        
        result
    }
    
    fn get_category_code(category_name: &str) -> &str {
        match category_name {
            "Critical dry" => "CD",
            "Lower fail" => "LF",
            "Marginal" => "MA",
            "Upper Fail" => "UF",
            "Cold and Humid" => "CH",
            "Lower optimal" => "LO",
            "Optimal" => "OP",
            "Upper Optimal" => "UO",
            "Upper Marginal" => "UM",
            "Upper Fail (high hum)" => "UH",
            "Critical" => "CR",
            _ => "??",
        }
    }   

    fn rle_to_string(&self, rle_data: &[RLEOutput]) -> String {
        rle_data.iter()
            .map(|rle| format!("{}{}", rle.quantity, Self::get_category_code(&rle.category_name)))
            .collect::<Vec<String>>()
            .join("")
    }
    
    // Resetar todos os dados
    fn reset(&self) {
        let mut sensor_data = self.sensor_data.lock().unwrap();
        let mut classified_data = self.classified_data.lock().unwrap();
        let mut rle_results = self.rle_results.lock().unwrap();
        
        sensor_data.clear();
        classified_data.clear();
        rle_results.clear();
    }
    
    // Adicionar dado do sensor e classificar
    fn add_sensor_data(&self, data: SensorData) -> ClassifiedData {
        // Adicionar aos dados brutos
        {
            let mut sensor_data = self.sensor_data.lock().unwrap();
            sensor_data.push(data.clone());
        }
        
        // Classificar com kNN
        let (category, category_name) = self.knn_classify(data.temperature, data.humidity, 3);
        
        let classified = ClassifiedData {
            category,
            category_name,
            temperature: data.temperature,
            humidity: data.humidity,
            timestamp: data.timestamp,
        };
        
        // Adicionar aos dados classificados
        {
            let mut classified_data = self.classified_data.lock().unwrap();
            classified_data.push(classified.clone());
        }
        
        classified
    }
    
    // Obter dados classificados
    fn get_classified_data(&self) -> Vec<ClassifiedData> {
        self.classified_data.lock().unwrap().clone()
    }
    
    // Aplicar RLE e retornar resultado
    fn get_rle_result(&self) -> (Vec<RLEOutput>, String, usize, usize) {
        let classified_data = self.classified_data.lock().unwrap();
        
        if classified_data.is_empty() {
            return (Vec::new(), String::new(), 0, 0);
        }
        
        let rle_result = self.apply_rle(&classified_data);
        let compressed = self.rle_to_string(&rle_result);
        let original_count = classified_data.len();
        let compressed_count = rle_result.len();
        
        // Salvar resultado
        {
            let mut rle_results = self.rle_results.lock().unwrap();
            *rle_results = rle_result.clone();
        }
        
        (rle_result, compressed, original_count, compressed_count)
    }
    
    // Obter estatísticas
    fn get_statistics(&self) -> serde_json::Value {
        let classified_data = self.classified_data.lock().unwrap();
        
        if classified_data.is_empty() {
            return serde_json::json!({"error": "Sem dados"});
        }
        
        // Contagem por categoria (como Tabela 2 do artigo)
        let mut category_count: HashMap<String, u32> = HashMap::new();
        for data in classified_data.iter() {
            *category_count.entry(data.category_name.clone()).or_insert(0) += 1;
        }
        
        // Estatísticas gerais
        let temps: Vec<f32> = classified_data.iter().map(|d| d.temperature).collect();
        let hums: Vec<f32> = classified_data.iter().map(|d| d.humidity).collect();
        
        let temp_mean = temps.iter().sum::<f32>() / temps.len() as f32;
        let hum_mean = hums.iter().sum::<f32>() / hums.len() as f32;
        
        let temp_std = (temps.iter().map(|&t| (t - temp_mean).powi(2)).sum::<f32>() / temps.len() as f32).sqrt();
        let hum_std = (hums.iter().map(|&h| (h - hum_mean).powi(2)).sum::<f32>() / hums.len() as f32).sqrt();
        
        serde_json::json!({
            "total_registros": classified_data.len(),
            "categorias": category_count,
            "temperatura": {
                "media": format!("{:.2}", temp_mean),
                "desvio": format!("{:.2}", temp_std),
                "min": temps.iter().fold(f32::INFINITY, |a, &b| a.min(b)),
                "max": temps.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b))
            },
            "umidade": {
                "media": format!("{:.2}", hum_mean),
                "desvio": format!("{:.2}", hum_std),
                "min": hums.iter().fold(f32::INFINITY, |a, &b| a.min(b)),
                "max": hums.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b))
            }
        })
    }
}

async fn handle_request(
    req: Request<Body>,
    state: Arc<AppState>,
) -> Result<Response<Body>, anyhow::Error> {
    match (req.method(), req.uri().path()) {
        (&Method::GET, "/") => Ok(Response::new(Body::from(
            "Endpoints disponíveis: /inserir (POST), /classificados (GET), /rle (GET), /estatisticas (GET), /reset (POST)",
        ))),

        (&Method::POST, "/inserir") => {
            println!("Requisição recebida: {} {}", req.method(), req.uri().path());
            
            let byte_stream = match hyper::body::to_bytes(req).await {
                Ok(bytes) => bytes,
                Err(e) => {
                    eprintln!("Erro ao ler corpo da requisição: {}", e);
                    let mut res = response_build(&serde_json::json!({"error": "Erro ao ler corpo da requisição"}).to_string());
                    *res.status_mut() = StatusCode::BAD_REQUEST;
                    return Ok(res);
                }
            };

            let sensor_data: SensorData = match serde_json::from_slice(&byte_stream) {
                Ok(data) => data,
                Err(e) => {
                    eprintln!("Erro de parsing JSON: {}", e);
                    let mut res = response_build(&serde_json::json!({"error": format!("Dados JSON inválidos: {}", e)}).to_string());
                    *res.status_mut() = StatusCode::BAD_REQUEST;
                    return Ok(res);
                }
            };

            // Validação básica dos dados
            if sensor_data.temperature.is_nan() || sensor_data.humidity.is_nan() {
                let mut res = response_build(&serde_json::json!({"error": "Valores numéricos não podem ser NaN"}).to_string());
                *res.status_mut() = StatusCode::BAD_REQUEST;
                return Ok(res);
            }

            if sensor_data.humidity < 0.0 || sensor_data.humidity > 100.0 {
                let mut res = response_build(&serde_json::json!({"error": "A umidade deve estar entre 0 e 100"}).to_string());
                *res.status_mut() = StatusCode::BAD_REQUEST;
                return Ok(res);
            }
            
            let classified = state.add_sensor_data(sensor_data.clone());
            
            println!("📥 Dados recebidos - Temp: {:.1}°C | Umidade: {:.1}% | Classificado: {}", 
                     sensor_data.temperature, sensor_data.humidity, classified.category_name);
            
            let mut res = response_build(&serde_json::json!({"status": "ok", "categoria": classified.category_name}).to_string());
            *res.status_mut() = StatusCode::CREATED;
            Ok(res)
        },

        (&Method::GET, "/classificados") => {
            let classified_data = state.get_classified_data();
            let json = serde_json::to_string(&classified_data)?;
            Ok(response_build(&json))
        },

        (&Method::GET, "/rle") => {
            let (rle_result, compressed, original_count, compressed_count) = state.get_rle_result();
            
            if rle_result.is_empty() {
                return Ok(response_build(&serde_json::json!({"error": "Sem dados para compressão"}).to_string()));
            }
            
            let reduction = if original_count > 0 {
                (1.0 - (compressed_count as f64 / original_count as f64)) * 100.0
            } else {
                0.0
            };
            
            println!("📦 RLE aplicado - {} grupos gerados", rle_result.len());
            println!("🔤 String comprimida: {}", compressed);
            println!("📊 Redução: {:.2}%", reduction);
            
            let response = serde_json::json!({
                "rle_data": rle_result,
                "compressed_string": compressed,
                "original_count": original_count,
                "compressed_count": compressed_count,
                "reduction": format!("{:.2}%", reduction)
            });
            
            Ok(response_build(&response.to_string()))
        },

        (&Method::GET, "/estatisticas") => {
            let stats = state.get_statistics();
            Ok(response_build(&stats.to_string()))
        },

        (&Method::POST, "/reset") => {
            state.reset();
            println!("🔄 Dados resetados");
            Ok(response_build(&serde_json::json!({"status": "resetado"}).to_string()))
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
    let porta_str = args.get(1)
        .cloned()
        .unwrap_or_else(|| env::var("PORTA").unwrap_or_else(|_| "8081".to_string()));
    let porta: u16 = porta_str.parse().expect("Porta inválida");

    let addr = SocketAddr::from(([0, 0, 0, 0], porta));
    
    // Criar estado compartilhado
    let state = Arc::new(AppState::new());
    
    // URL do Receiver (Cloud): argumento 2 ou env RECEIVER_URL ou default
    let receiver_url = args.get(2)
        .cloned()
        .unwrap_or_else(|| env::var("RECEIVER_URL").unwrap_or_else(|_| "http://localhost:8082/receber".to_string()));

    // Ambiente: argumento 3 ou env AMBIENTE ou Fog
    let ambiente = args.get(3)
        .cloned()
        .unwrap_or_else(|| env::var("AMBIENTE").unwrap_or_else(|_| "Fog".to_string()));

    let addr = SocketAddr::from(([0, 0, 0, 0], porta));
    
    // Criar estado compartilhado
    let state = Arc::new(AppState::new());
    
    // Tarefa em segundo plano para envio periódico ao Receiver
    let state_clone = state.clone();
    let receiver_url_clone = receiver_url.clone();
    let ambiente_clone = ambiente.clone();
    tokio::spawn(async move {
        let client = reqwest::Client::new();
        let interval = tokio::time::Duration::from_secs(30);
        
        loop {
            tokio::time::sleep(interval).await;
            
            let (rle_result, compressed, original_count, compressed_count) = state_clone.get_rle_result();
            
            if !rle_result.is_empty() {
                let reduction_val = if original_count > 0 {
                    (1.0 - (compressed_count as f64 / original_count as f64)) * 100.0
                } else {
                    0.0
                };
                
                let payload = RLEPayload {
                    rle_data: rle_result,
                    compressed_string: compressed,
                    original_count,
                    compressed_count,
                    reduction: format!("{:.2}%", reduction_val),
                };

                println!("📤 {} [Fog]: Enviando dados RLE para o Receiver...", ambiente_clone);
                let res: Result<reqwest::Response, reqwest::Error> = client.post(&receiver_url_clone)
                    .json(&payload)
                    .send()
                    .await;

                match res {
                    Ok(resp) => {
                        if resp.status().is_success() {
                            println!("✅ Dados enviados com sucesso. Limpando armazenamento local...");
                            state_clone.reset();
                        } else {
                            eprintln!("⚠️ Erro na resposta: {}", resp.status());
                        }
                    }
                    Err(e) => {
                        eprintln!("❌ Falha ao conectar: {}", e);
                    }
                }
            } else {
                println!("⏳ Fog [{}]: Sem dados para enviar neste ciclo.", ambiente_clone);
            }
        }
    });

    let make_svc = make_service_fn(move |_| {
        let state = state.clone();
        async move {
            Ok::<_, Infallible>(service_fn(move |req| {
                let state = state.clone();
                handle_request(req, state)
            }))
        }
    });

    println!("==========================================");
    println!("🚀 Filtro rodando na porta {}", porta);
    println!("🌍 Ambiente: {}", ambiente);
    println!("☁️  Receiver alvo: {}", receiver_url);
    println!("⏱️  Ciclo de envio: 30 segundos");
    println!("==========================================");
    println!("Endpoints:");
    println!("  POST /inserir     - Recebe dados dos sensores");
    println!("  GET  /classificados - Lista dados classificados");
    println!("  GET  /rle         - Aplica RLE e retorna compressão");
    println!("  GET  /estatisticas - Estatísticas dos dados");
    println!("  POST /reset       - Limpa todos os dados");
    println!("==========================================");

    let server = Server::bind(&addr).serve(make_svc);

    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }
   
    Ok(())
}