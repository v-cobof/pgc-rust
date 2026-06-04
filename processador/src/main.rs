use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, StatusCode, Server};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::collections::HashMap;
use std::env;
use std::sync::{Arc, Mutex};
use serde::{Deserialize, Serialize};

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

// Estrutura auxiliar para calcular o tamanho_original_bytes (Opção B)
#[derive(Debug, Serialize, Clone)]
struct TempHum {
    temperature: f32,
    humidity: f32,
}

// Estruturas para a exportação de estatísticas
#[derive(Debug, Serialize, Clone)]
struct Metadados {
    total_registros: usize,
    knn_k: usize,
    timestamp_processamento: String,
}

#[derive(Debug, Serialize, Clone)]
struct EstatisticasGlobais {
    tamanho_original_bytes: usize,
    tamanho_knn_bytes: usize,
    tamanho_knn_rle_bytes: usize,
    taxa_compressao_knn_percent: f64,
    taxa_compressao_knn_rle_percent: f64,
}

#[derive(Debug, Serialize, Clone)]
struct LoteOutput {
    lote_id: usize,
    timestamp: String,
    registros_originais: usize,
    tamanho_original_bytes: usize,
    sequencia_categorias: Vec<String>,
    rle_segments: Vec<(String, u32)>,
    tamanho_knn_bytes: usize,
    tamanho_knn_rle_bytes: usize,
    taxa_compressao_knn: f64,
    taxa_compressao_knn_rle: f64,
}

#[derive(Debug, Serialize, Clone)]
struct RelatorioEstatisticas {
    metadados: Metadados,
    estatisticas_globais: EstatisticasGlobais,
    series_temporais: Vec<LoteOutput>,
    contagem_categorias: HashMap<String, u32>,
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
            "Critical dry" => "D",
            "Lower fail" => "F",
            "Marginal" => "M",
            "Upper Fail" => "U",
            "Cold and Humid" => "H",
            "Lower optimal" => "L",
            "Optimal" => "O",
            "Upper Optimal" => "P",
            "Upper Marginal" => "G",
            "Upper Fail (high hum)" => "I",
            "Critical" => "R",
            _ => "?",
        }
    }   

    fn rle_to_string(&self, rle_data: &[RLEOutput]) -> String {
        rle_data.iter()
            .map(|rle| format!("{}{}", rle.quantity, Self::get_category_code(&rle.category_name)))
            .collect::<Vec<String>>()
            .join("")
    }

    // Fazer RLE em uma string de códigos (ex.: "OOOPPPGGG") -> "3O3P3G"
    fn rle_codes_string(codes: &str) -> String {
        if codes.is_empty() {
            return String::new();
        }

        let mut result = String::new();
        let mut chars = codes.chars();
        let mut current = match chars.next() {
            Some(c) => c,
            None => return result,
        };
        let mut count: usize = 1;

        for c in chars {
            if c == current {
                count += 1;
            } else {
                result.push_str(&format!("{}{}", count, current));
                current = c;
                count = 1;
            }
        }

        // último grupo
        result.push_str(&format!("{}{}", count, current));
        result
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

    // Obter estatísticas de compressão em bytes (redução real)
    fn get_byte_compression_metrics(&self) -> Option<(usize, usize, usize, f64, f64)> {
        let classified_data = self.classified_data.lock().unwrap();
        if classified_data.is_empty() {
            return None;
        }
        
        let vec_temp_hum: Vec<TempHum> = classified_data.iter()
            .map(|c| TempHum { temperature: c.temperature, humidity: c.humidity })
            .collect();
        let serialized_original = serde_json::to_string(&vec_temp_hum).unwrap_or_default();
        let tamanho_original_bytes = serialized_original.len();
        
        let sequencia_categorias: Vec<String> = classified_data.iter()
            .map(|c| Self::get_category_code(&c.category_name).to_string())
            .collect();
        let string_knn = sequencia_categorias.join("");
        let tamanho_knn_bytes = string_knn.len();
        
        let rle_result = self.apply_rle(&classified_data);
        let string_knn_rle = self.rle_to_string(&rle_result);
        let tamanho_knn_rle_bytes = string_knn_rle.len();
        
        let taxa_compressao_knn = if tamanho_original_bytes > 0 {
            (1.0 - (tamanho_knn_bytes as f64 / tamanho_original_bytes as f64)) * 100.0
        } else {
            0.0
        };
        
        let taxa_compressao_knn_rle = if tamanho_original_bytes > 0 {
            (1.0 - (tamanho_knn_rle_bytes as f64 / tamanho_original_bytes as f64)) * 100.0
        } else {
            0.0
        };
        
        Some((tamanho_original_bytes, tamanho_knn_bytes, tamanho_knn_rle_bytes, taxa_compressao_knn, taxa_compressao_knn_rle))
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

fn get_estatisticas(relatorio: &RelatorioEstatisticas) -> String {
    serde_json::to_string_pretty(relatorio).unwrap_or_default()
}

fn export_para_json(relatorio: &RelatorioEstatisticas, caminho_arquivo: &str) -> std::io::Result<()> {
    use std::fs::File;
    use std::io::Write;
    let json_str = get_estatisticas(relatorio);
    let mut file = File::create(caminho_arquivo)?;
    file.write_all(json_str.as_bytes())?;
    Ok(())
}

#[derive(Debug, Deserialize)]
struct CsvRecord {
    temperatura: f32,
    umidade: f32,
}

fn run_batch_mode(csv_path: &str, output_path: &str, state: &AppState, global_rle: bool) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("📖 Carregando dados de {}...", csv_path);
    
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_path(csv_path)?;
        
    let records: Vec<CsvRecord> = reader.deserialize()
        .collect::<Result<Vec<CsvRecord>, csv::Error>>()?;
        
    println!("✅ {} registros carregados do CSV.", records.len());
    
    let mut series_temporais = Vec::new();
    let mut contagem_categorias = HashMap::new();
    let categorias_validas = vec!["D", "F", "M", "U", "H", "L", "O", "P", "G", "I", "R"];
    for cat in &categorias_validas {
        contagem_categorias.insert(cat.to_string(), 0);
    }
    
    let mut global_tamanho_original = 0;
    let mut global_tamanho_knn = 0;
    let mut global_tamanho_knn_rle = 0;
    let mut global_string_knn = String::new();
    
    let total_registros = records.len();
    
    for (i, lote_records) in records.chunks(6).enumerate() {
        let lote_id = i + 1;
        let registros_originais = lote_records.len();
        
        let mut lote_temp_hum = Vec::new();
        let mut lote_classified_data = Vec::new();
        
        for record in lote_records {
            lote_temp_hum.push(TempHum {
                temperature: record.temperatura,
                humidity: record.umidade,
            });
            
            let (category_id, category_name) = state.knn_classify(record.temperatura, record.umidade, 3);
            lote_classified_data.push(ClassifiedData {
                category: category_id,
                category_name: category_name.clone(),
                temperature: record.temperatura,
                humidity: record.umidade,
                timestamp: 0,
            });
        }
        
        let serialized_original = serde_json::to_string(&lote_temp_hum)?;
        let tamanho_original_bytes = serialized_original.len();
        global_tamanho_original += tamanho_original_bytes;
        
        let sequencia_categorias: Vec<String> = lote_classified_data.iter()
            .map(|c| AppState::get_category_code(&c.category_name).to_string())
            .collect();
            
        for sigla in &sequencia_categorias {
            *contagem_categorias.entry(sigla.clone()).or_insert(0) += 1;
        }
        
        let string_knn = sequencia_categorias.join("");
        let tamanho_knn_bytes = string_knn.len();
        global_tamanho_knn += tamanho_knn_bytes;
        if global_rle {
            global_string_knn.push_str(&string_knn);
        }
        
        let rle_result = state.apply_rle(&lote_classified_data);
        
        let rle_segments: Vec<(String, u32)> = rle_result.iter()
            .map(|rle| (AppState::get_category_code(&rle.category_name).to_string(), rle.quantity))
            .collect();
            
        let string_knn_rle = state.rle_to_string(&rle_result);
        let tamanho_knn_rle_bytes = string_knn_rle.len();
        global_tamanho_knn_rle += tamanho_knn_rle_bytes;
        
        let taxa_compressao_knn = if tamanho_original_bytes > 0 {
            (1.0 - (tamanho_knn_bytes as f64 / tamanho_original_bytes as f64)) * 100.0
        } else {
            0.0
        };
        
        let taxa_compressao_knn_rle = if tamanho_original_bytes > 0 {
            (1.0 - (tamanho_knn_rle_bytes as f64 / tamanho_original_bytes as f64)) * 100.0
        } else {
            0.0
        };
        
        let timestamp_lote = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
        
        series_temporais.push(LoteOutput {
            lote_id,
            timestamp: timestamp_lote,
            registros_originais,
            tamanho_original_bytes,
            sequencia_categorias,
            rle_segments,
            tamanho_knn_bytes,
            tamanho_knn_rle_bytes,
            taxa_compressao_knn,
            taxa_compressao_knn_rle,
        });
    }
    
    let taxa_compressao_knn_percent = if global_tamanho_original > 0 {
        (1.0 - (global_tamanho_knn as f64 / global_tamanho_original as f64)) * 100.0
    } else {
        0.0
    };

    // Se solicitado, calcule o RLE sobre a cadeia global concatenada (maximizar impacto do RLE)
    if global_rle {
        let global_rle_string = AppState::rle_codes_string(&global_string_knn);
        global_tamanho_knn_rle = global_rle_string.len();
    }

    let taxa_compressao_knn_rle_percent = if global_tamanho_original > 0 {
        (1.0 - (global_tamanho_knn_rle as f64 / global_tamanho_original as f64)) * 100.0
    } else {
        0.0
    };
    
    let estatisticas_globais = EstatisticasGlobais {
        tamanho_original_bytes: global_tamanho_original,
        tamanho_knn_bytes: global_tamanho_knn,
        tamanho_knn_rle_bytes: global_tamanho_knn_rle,
        taxa_compressao_knn_percent,
        taxa_compressao_knn_rle_percent,
    };
    
    let metadados = Metadados {
        total_registros,
        knn_k: 3,
        timestamp_processamento: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
    };
    
    let relatorio = RelatorioEstatisticas {
        metadados,
        estatisticas_globais,
        series_temporais,
        contagem_categorias,
    };
    
    export_para_json(&relatorio, output_path)?;
    
    println!("==========================================");
    println!("📊 Relatório de Processamento em Lote Gerado!");
    println!("📁 Arquivo salvo em: {}", output_path);
    println!("📈 Total de registros processados: {}", total_registros);
    println!("📦 Tamanho original acumulado: {} bytes", global_tamanho_original);
    println!("🏷️  Tamanho após kNN: {} bytes (Redução: {:.2}%)", global_tamanho_knn, taxa_compressao_knn_percent);
    println!("🗜️  Tamanho após kNN+RLE: {} bytes (Redução: {:.2}%)", global_tamanho_knn_rle, taxa_compressao_knn_rle_percent);
    println!("==========================================");
    
    Ok(())
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
            
            let reduction = if let Some((_, _, _, _, taxa_rle)) = state.get_byte_compression_metrics() {
                taxa_rle
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
    
    // Se o argumento 1 for "batch", executa em lote offline e encerra
    if args.get(1).map(|s| s.as_str()) == Some("batch") {
        let csv_path = args.get(2).map(|s| s.as_str()).unwrap_or("entrada.csv");
        let output_path = args.get(3).map(|s| s.as_str()).unwrap_or("estatisticas_compressao.json");
        // Detectar flag --global-rle em qualquer posição dos args
        let use_global_rle = args.iter().any(|s| s == "--global-rle");
        let state = AppState::new();
        run_batch_mode(csv_path, output_path, &state, use_global_rle)?;
        return Ok(());
    }

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
                let reduction_val = if let Some((_, _, _, _, taxa_rle)) = state_clone.get_byte_compression_metrics() {
                    taxa_rle
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

                if ambiente_clone == "Thing" {
                    println!("📤 {} [Thing]: Enviando dados RLE para o LoRa relay...", ambiente_clone);
                } else {
                    println!("📤 {} [Fog]: Enviando dados RLE para o Receiver...", ambiente_clone);
                }
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