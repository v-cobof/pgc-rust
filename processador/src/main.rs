use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, StatusCode, Server};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::collections::HashMap;
use std::env;
use std::sync::{Arc, Mutex};
use serde::{Deserialize, Serialize};
use rand::Rng;

// ==============================================================================
// ESTRUTURAS DE DADOS DO ECOSSISTEMA
// ==============================================================================

// Estrutura que representa a leitura bruta de um sensor (enviada via HTTP/JSON).
#[derive(Debug, Deserialize, Serialize, Clone)]
struct SensorData {
    temperature: f32, // Leitura de Temperatura física (Celsius)
    humidity: f32,    // Leitura de Umidade Relativa do ar (%)
    timestamp: u64,   // Época Unix em segundos da captura
}

// Estrutura que estende a leitura bruta adicionando a classificação agronômica correspondente.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ClassifiedData {
    category: u32,             // ID Numérico da categoria
    category_name: String,     // Nome textual da categoria agronômica
    temperature: f32,          // Temperatura física original
    humidity: f32,             // Umidade física original
    timestamp: u64,            // Timestamp da medição
}

// Representa um registro do dataset de treinamento para parametrização do kNN.
#[derive(Debug, Clone)]
struct TrainingSample {
    category: u32,
    category_name: String,
    temp_min: f32, // Limite inferior de temperatura da categoria
    temp_max: f32, // Limite superior de temperatura da categoria
    hum_min: f32,  // Limite inferior de umidade da categoria
    hum_max: f32,  // Limite superior de umidade da categoria
}

// Estrutura que descreve um bloco contíguo após compressão Run-Length Encoding (RLE).
#[derive(Debug, Serialize, Deserialize, Clone)]
struct RLEOutput {
    category_name: String, // Nome da categoria que se repete
    quantity: u32,         // Quantidade de ocorrências consecutivas
}

// Payload JSON final pronto para ser transmitido ao Cloud/Receiver.
#[derive(Debug, Serialize, Deserialize, Clone)]
struct RLEPayload {
    rle_data: Vec<RLEOutput>,   // Vetor contendo a cadeia estruturada de repetições
    compressed_string: String,  // Representação textual compactada (ex: "3O2F")
    original_count: usize,      // Total de leituras físicas originais no lote
    compressed_count: usize,    // Total de segmentos compactados
    reduction: String,          // Ganho de compressão expresso em porcentagem
}

// Estrutura compacta contendo apenas as leituras de sensores para cálculo do payload original.
#[derive(Debug, Serialize, Clone)]
struct TempHum {
    temperature: f32,
    humidity: f32,
}

// ==============================================================================
// ESTRUTURAS PARA EXPORTAÇÃO E GERAÇÃO DE RELATÓRIO JSON (MODO LOTE/OFFLINE)
// ==============================================================================

// Metadados básicos sobre a execução do processamento em lote.
#[derive(Debug, Serialize, Clone)]
struct Metadados {
    total_registros: usize,          // Total de linhas processadas
    knn_k: usize,                    // Valor de vizinhos mais próximos usado
    timestamp_processamento: String, // Data/Hora ISO do processamento
}

// Métricas de taxa de redução e consumo de bytes no nível global (toda a série).
#[derive(Debug, Serialize, Clone)]
struct EstatisticasGlobais {
    tamanho_original_bytes: usize,       // Tamanho em bytes da representação JSON original
    tamanho_knn_bytes: usize,            // Tamanho em bytes usando kNN Puro (1 char/registro)
    tamanho_knn_rle_bytes: usize,        // Tamanho em bytes após aplicar RLE na sequência kNN
    taxa_compressao_knn_percent: f64,    // Taxa de redução do kNN Puro em relação ao JSON original (%)
    taxa_compressao_knn_rle_percent: f64,// Taxa de redução do kNN+RLE em relação ao JSON original (%)
}

// Representa as métricas e dados de um lote/pacote temporal individualmente.
#[derive(Debug, Serialize, Clone)]
struct LoteOutput {
    lote_id: usize,                  // ID sequencial do lote
    timestamp: String,               // Timestamp do processamento do lote
    registros_originais: usize,      // Quantidade de leituras contidas neste lote
    tamanho_original_bytes: usize,   // Bytes consumidos pelo JSON original bruto
    sequencia_categorias: Vec<String>, // Lista das siglas kNN associadas (ex: ["O", "O", "F"])
    rle_segments: Vec<(String, u32)>,  // Lista de tuplas comprimidas RLE (categoria, repetição)
    tamanho_knn_bytes: usize,        // Bytes consumidos usando kNN Puro
    tamanho_knn_rle_bytes: usize,    // Bytes consumidos usando kNN+RLE
    taxa_compressao_knn: f64,        // Redução do kNN Puro para este lote (%)
    taxa_compressao_knn_rle: f64,    // Redução do kNN+RLE para este lote (%)
}

// Estrutura de agregação do relatório estatístico completo de lote.
#[derive(Debug, Serialize, Clone)]
struct RelatorioEstatisticas {
    metadados: Metadados,
    estatisticas_globais: EstatisticasGlobais,
    series_temporais: Vec<LoteOutput>,
    contagem_categorias: HashMap<String, u32>, // Distribuição final das classes
}

// ==============================================================================
// ESTADO COMPARTILHADO DA APLICAÇÃO (THREADS)
// ==============================================================================
// Armazena as bases em memória protegidas por Mutex para acesso seguro entre threads.
struct AppState {
    sensor_data: Mutex<Vec<SensorData>>,          // Histórico de leituras brutas
    classified_data: Mutex<Vec<ClassifiedData>>,  // Histórico de leituras classificadas
    rle_results: Mutex<Vec<RLEOutput>>,          // Histórico das corridas RLE
    training_dataset: Vec<TrainingSample>,        // Base estática do modelo de cultivo
}

impl AppState {
    // Inicializa o estado com listas vazias e gera a base estática do tomateiro
    fn new() -> Self {
        Self {
            sensor_data: Mutex::new(Vec::new()),
            classified_data: Mutex::new(Vec::new()),
            rle_results: Mutex::new(Vec::new()),
            training_dataset: Self::create_training_dataset(),
        }
    }
    
    // Constrói o modelo de treino estático para classificação (Tabela do Tomateiro)
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
    
    // Normaliza linearmente um valor (Min-Max) para a escala [0, 1]
    fn normalize_value(&self, value: f32, min: f32, max: f32) -> f32 {
        (value - min) / (max - min)
    }
    
    // Calcula a distância euclidiana simples entre dois pontos bidimensionais
    fn euclidean_distance(&self, t1: f32, h1: f32, t2: f32, h2: f32) -> f32 {
        ((t1 - t2).powi(2) + (h1 - h2).powi(2)).sqrt()
    }
    
    // Implementa a classificação kNN em escala normalizada.
    // Classifica uma leitura (temperatura, umidade) para a classe agronômica dominante
    // entre os k vizinhos mais próximos definidos no modelo estático.
    fn knn_classify(&self, temperature: f32, humidity: f32, k: usize) -> (u32, String) {
        // Mapeia os limites extremos globais da base de dados de treinamento para normalização
        let all_temps: Vec<f32> = self.training_dataset.iter().flat_map(|t| vec![t.temp_min, t.temp_max]).collect();
        let all_hums: Vec<f32> = self.training_dataset.iter().flat_map(|t| vec![t.hum_min, t.hum_max]).collect();
        let temp_min = all_temps.iter().fold(f32::INFINITY, |a, &b| a.min(b));
        let temp_max = all_temps.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
        let hum_min = all_hums.iter().fold(f32::INFINITY, |a, &b| a.min(b));
        let hum_max = all_hums.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
        
        // Normaliza a leitura atual
        let norm_temp = self.normalize_value(temperature, temp_min, temp_max);
        let norm_hum = self.normalize_value(humidity, hum_min, hum_max);
        
        // Calcula a distância para o centroide de cada classe
        let mut distances: Vec<(f32, u32, String)> = self.training_dataset.iter().map(|sample| {
            let sample_temp_mid = (sample.temp_min + sample.temp_max) / 2.0;
            let sample_hum_mid = (sample.hum_min + sample.hum_max) / 2.0;
            let norm_sample_temp = self.normalize_value(sample_temp_mid, temp_min, temp_max);
            let norm_sample_hum = self.normalize_value(sample_hum_mid, hum_min, hum_max);
            
            let dist = self.euclidean_distance(norm_temp, norm_hum, norm_sample_temp, norm_sample_hum);
            (dist, sample.category, sample.category_name.clone())
        }).collect();
        
        // Ordena pela menor distância
        distances.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        
        // Seleciona os k vizinhos mais próximos
        let neighbors = &distances[0..k.min(distances.len())];
        
        // Computa os votos de cada classe presente na vizinhança
        let mut votes: HashMap<u32, (u32, String)> = HashMap::new();
        for &(_, cat, ref name) in neighbors {
            let entry = votes.entry(cat).or_insert((0, name.clone()));
            entry.0 += 1;
        }
        
        // Determina e retorna a categoria majoritária
        let (category, (_, name)) = votes.into_iter()
            .max_by_key(|&(_, (count, _))| count)
            .unwrap_or((0, (1, "Unknown".to_string())));
        
        (category, name)
    }
    
    // Implementa o algoritmo de codificação por comprimento de corrida (RLE - Run Length Encoding).
    // Agrupa sequências repetidas consecutivas em blocos simplificados (Classe, Repetição).
    fn apply_rle(&self, data: &[ClassifiedData]) -> Vec<RLEOutput> {
        if data.is_empty() {
            return Vec::new();
        }
        
        let mut result = Vec::new();
        let mut current_category = &data[0].category_name;
        let mut current_count = 1;
        
        // Itera sobre as leituras subsequentes acumulando contagens
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
        
        // Insere o segmento final
        result.push(RLEOutput {
            category_name: current_category.clone(),
            quantity: current_count,
        });
        
        result
    }
    
    // Converte o nome extenso da categoria agronômica em uma sigla de 1 caractere (1 byte)
    // para viabilizar a compressão de dados na borda da rede.
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

    // Formata o resultado do RLE estruturado em uma string comprimida (ex.: "3O2P1H")
    fn rle_to_string(&self, rle_data: &[RLEOutput]) -> String {
        rle_data.iter()
            .map(|rle| format!("{}{}", rle.quantity, Self::get_category_code(&rle.category_name)))
            .collect::<Vec<String>>()
            .join("")
    }

    // Executa codificação RLE sobre uma sequência puramente textual de siglas (ex.: "OOOPP" -> "3O2P")
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

        // Adiciona o segmento remanescente
        result.push_str(&format!("{}{}", count, current));
        result
    }
    
    // Limpa de forma segura todos os históricos de dados armazenados localmente na memória (Buffer reset)
    fn reset(&self) {
        let mut sensor_data = self.sensor_data.lock().unwrap();
        let mut classified_data = self.classified_data.lock().unwrap();
        let mut rle_results = self.rle_results.lock().unwrap();
        
        sensor_data.clear();
        classified_data.clear();
        rle_results.clear();
    }
    
    // Insere uma nova leitura de sensor na memória e dispara o classificador kNN para rotulá-la
    fn add_sensor_data(&self, data: SensorData) -> ClassifiedData {
        // Armazena a leitura bruta no vetor correspondente
        {
            let mut sensor_data = self.sensor_data.lock().unwrap();
            sensor_data.push(data.clone());
        }
        
        // Executa a classificação usando kNN com k=3
        let (category, category_name) = self.knn_classify(data.temperature, data.humidity, 3);
        
        let classified = ClassifiedData {
            category,
            category_name,
            temperature: data.temperature,
            humidity: data.humidity,
            timestamp: data.timestamp,
        };
        
        // Armazena a leitura classificada
        {
            let mut classified_data = self.classified_data.lock().unwrap();
            classified_data.push(classified.clone());
        }
        
        classified
    }
    
    // Retorna uma cópia thread-safe das leituras classificadas registradas
    fn get_classified_data(&self) -> Vec<ClassifiedData> {
        self.classified_data.lock().unwrap().clone()
    }

    // Calcula e avalia as taxas exatas de redução de bytes no nível de rede para os
    // algoritmos kNN Puro e kNN+RLE em relação à serialização JSON original dos sensores.
    fn get_byte_compression_metrics(&self) -> Option<(usize, usize, usize, f64, f64)> {
        let classified_data = self.classified_data.lock().unwrap();
        if classified_data.is_empty() {
            return None;
        }
        
        // Calcula o tamanho original simulando a codificação padrão JSON estruturada
        let vec_temp_hum: Vec<TempHum> = classified_data.iter()
            .map(|c| TempHum { temperature: c.temperature, humidity: c.humidity })
            .collect();
        let serialized_original = serde_json::to_string(&vec_temp_hum).unwrap_or_default();
        let tamanho_original_bytes = serialized_original.len();
        
        // Calcula o tamanho consumido pelo kNN Puro (concatenação simples de caracteres de 1 byte)
        let sequencia_categorias: Vec<String> = classified_data.iter()
            .map(|c| Self::get_category_code(&c.category_name).to_string())
            .collect();
        let string_knn = sequencia_categorias.join("");
        let tamanho_knn_bytes = string_knn.len();
        
        // Calcula o tamanho consumido após a codificação RLE
        let rle_result = self.apply_rle(&classified_data);
        let string_knn_rle = self.rle_to_string(&rle_result);
        let tamanho_knn_rle_bytes = string_knn_rle.len();
        
        // Determina os percentuais de redução
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
    
    // Executa a codificação RLE no histórico corrente e retorna as contagens brutas e a string
    fn get_rle_result(&self) -> (Vec<RLEOutput>, String, usize, usize) {
        let classified_data = self.classified_data.lock().unwrap();
        
        if classified_data.is_empty() {
            return (Vec::new(), String::new(), 0, 0);
        }
        
        let rle_result = self.apply_rle(&classified_data);
        let compressed = self.rle_to_string(&rle_result);
        let original_count = classified_data.len();
        let compressed_count = rle_result.len();
        
        // Atualiza os resultados em cache local
        {
            let mut rle_results = self.rle_results.lock().unwrap();
            *rle_results = rle_result.clone();
        }
        
        (rle_result, compressed, original_count, compressed_count)
    }
    
    // Computa as estatísticas gerais do histórico (média, desvio padrão, mínimos, máximos)
    // tanto para temperatura quanto para umidade relativa, retornando em JSON formatado.
    fn get_statistics(&self) -> serde_json::Value {
        let classified_data = self.classified_data.lock().unwrap();
        
        if classified_data.is_empty() {
            return serde_json::json!({"error": "Sem dados"});
        }
        
        // Contagem de ocorrências por categoria agronômica
        let mut category_count: HashMap<String, u32> = HashMap::new();
        for data in classified_data.iter() {
            *category_count.entry(data.category_name.clone()).or_insert(0) += 1;
        }
        
        // Coleta vetores numéricos das medições físicas
        let temps: Vec<f32> = classified_data.iter().map(|d| d.temperature).collect();
        let hums: Vec<f32> = classified_data.iter().map(|d| d.humidity).collect();
        
        // Calcula as médias aritméticas
        let temp_mean = temps.iter().sum::<f32>() / temps.len() as f32;
        let hum_mean = hums.iter().sum::<f32>() / hums.len() as f32;
        
        // Calcula os desvios padrão amostrais
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

// Prepara e formata o relatório estatístico final de lote em formato JSON indentado
fn get_estatisticas(relatorio: &RelatorioEstatisticas) -> String {
    serde_json::to_string_pretty(relatorio).unwrap_or_default()
}

// Exporta o JSON do relatório estatístico em lote para o caminho em disco fornecido
fn export_para_json(relatorio: &RelatorioEstatisticas, caminho_arquivo: &str) -> std::io::Result<()> {
    use std::fs::File;
    use std::io::Write;
    let json_str = get_estatisticas(relatorio);
    let mut file = File::create(caminho_arquivo)?;
    file.write_all(json_str.as_bytes())?;
    Ok(())
}

// Estrutura para leitura e parsing das colunas de sensor do CSV
#[derive(Debug, Deserialize)]
struct CsvRecord {
    temperatura: f32,
    umidade: f32,
}

// Gera números aleatórios com distribuição normal padrão Z ~ N(0, 1) em Rust
// utilizando a Transformada de Box-Muller para emular o gerador estocástico
// de forma autossuficiente, contornando o isolamento do sandbox WASM (WASI).
fn next_gaussian<R: rand::Rng>(rng: &mut R) -> f32 {
    let mut u1: f32 = rng.gen_range(0.0..1.0);
    // u1 deve pertencer ao intervalo semiaberto (0, 1] para evitar o logaritmo natural de zero ln(0)
    while u1 <= 0.0 {
        u1 = rng.gen_range(0.0..1.0);
    }
    let u2: f32 = rng.gen_range(0.0..1.0);
    
    // Aplicação da fórmula matemática da transformada de Box-Muller
    (-2.0 * u1.ln()).sqrt() * (2.0 * std::f32::consts::PI * u2).cos()
}

// Processador em lote (Batch Mode). Avalia o pipeline offline.
// Permite emular leituras de sensores via gerador estocástico KDE baseado no arquivo CSV real
// e calcular a eficiência de compressão para múltiplos tamanhos de lote temporais.
fn run_batch_mode(csv_path: &str, output_path: &str, state: &AppState, global_rle: bool, use_synthetic: bool, lote_tamanho: usize) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("📖 Carregando dados base de {}...", csv_path);
    
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_path(csv_path)?;
        
    let mut records: Vec<CsvRecord> = reader.deserialize()
        .collect::<Result<Vec<CsvRecord>, csv::Error>>()?;
        
    println!("✅ {} registros carregados do CSV base.", records.len());
    
    // Se ativado o sinalizador --synthetic, o processador gera 40.000 amostras
    // sintéticas a partir do dataset original usando Estimativa de Densidade por Kernel (KDE)
    if use_synthetic {
        println!("🎲 Gerando 40000 registros sintéticos usando KDE (bandwidth = 0.5)...");
        if records.is_empty() {
            return Err("Não há registros no CSV base para gerar dados sintéticos.".into());
        }
        
        let mut rng = rand::thread_rng();
        let mut synthetic_records = Vec::with_capacity(40000);
        
        for _ in 0..40000 {
            // Escolhe aleatoriamente um ponto base real (bootstrap)
            let idx = rng.gen_range(0..records.len());
            let base = &records[idx];
            
            // Adiciona ruído gaussiano baseado no bandwidth h=0.5
            let temp_noise = next_gaussian(&mut rng) * 0.5;
            let hum_noise = next_gaussian(&mut rng) * 0.5;
            
            synthetic_records.push(CsvRecord {
                temperatura: base.temperatura + temp_noise,
                umidade: base.umidade + hum_noise,
            });
        }
        
        // Grava as amostras sintéticas geradas em um novo arquivo CSV para validação estatística posterior
        println!("💾 Salvando dados sintéticos gerados em 'dados_sinteticos_gerados_rust.csv'...");
        let mut wtr = csv::WriterBuilder::new()
            .has_headers(true)
            .from_path("dados_sinteticos_gerados_rust.csv")?;
        wtr.write_record(&["temperatura", "umidade"])?;
        for r in &synthetic_records {
            wtr.write_record(&[r.temperatura.to_string(), r.umidade.to_string()])?;
        }
        wtr.flush()?;
        println!("✅ Arquivo 'dados_sinteticos_gerados_rust.csv' salvo com sucesso.");
        
        records = synthetic_records;
    }
    
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
    
    for (i, lote_records) in records.chunks(lote_tamanho).enumerate() {
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

// ==============================================================================
// HANDLER HTTP PRINCIPAL (ROTAS E ENDPOINTS)
// ==============================================================================
// Roteia as requisições HTTP para os respectivos endpoints da névoa de computação.
async fn handle_request(
    req: Request<Body>,
    state: Arc<AppState>,
) -> Result<Response<Body>, anyhow::Error> {
    match (req.method(), req.uri().path()) {
        // Rota GET /: Exibe a lista de endpoints disponíveis no processador
        (&Method::GET, "/") => Ok(Response::new(Body::from(
            "Endpoints disponíveis: /inserir (POST), /classificados (GET), /rle (GET), /estatisticas (GET), /reset (POST)",
        ))),

        // Rota POST /inserir: Recebe a leitura de um sensor, valida os valores,
        // realiza a classificação kNN local e insere nos buffers em memória.
        (&Method::POST, "/inserir") => {
            println!("Requisição recebida: {} {}", req.method(), req.uri().path());
            
            // Lê de forma assíncrona os bytes brutos do payload HTTP
            let byte_stream = match hyper::body::to_bytes(req).await {
                Ok(bytes) => bytes,
                Err(e) => {
                    eprintln!("Erro ao ler corpo da requisição: {}", e);
                    let mut res = response_build(&serde_json::json!({"error": "Erro ao ler corpo da requisição"}).to_string());
                    *res.status_mut() = StatusCode::BAD_REQUEST;
                    return Ok(res);
                }
            };

            // Deserializa o JSON contendo as chaves 'temperature', 'humidity', 'timestamp'
            let sensor_data: SensorData = match serde_json::from_slice(&byte_stream) {
                Ok(data) => data,
                Err(e) => {
                    eprintln!("Erro de parsing JSON: {}", e);
                    let mut res = response_build(&serde_json::json!({"error": format!("Dados JSON inválidos: {}", e)}).to_string());
                    *res.status_mut() = StatusCode::BAD_REQUEST;
                    return Ok(res);
                }
            };

            // Validação de dados físicos do sensor: impede valores indefinidos (NaN)
            if sensor_data.temperature.is_nan() || sensor_data.humidity.is_nan() {
                let mut res = response_build(&serde_json::json!({"error": "Valores numéricos não podem ser NaN"}).to_string());
                *res.status_mut() = StatusCode::BAD_REQUEST;
                return Ok(res);
            }

            // Validação física: a umidade relativa do ar deve estar no intervalo lógico [0%, 100%]
            if sensor_data.humidity < 0.0 || sensor_data.humidity > 100.0 {
                let mut res = response_build(&serde_json::json!({"error": "A umidade deve estar entre 0 e 100"}).to_string());
                *res.status_mut() = StatusCode::BAD_REQUEST;
                return Ok(res);
            }
            
            // Insere no buffer e obtém a classificação agronômica resultante do kNN (k=3)
            let classified = state.add_sensor_data(sensor_data.clone());
            
            println!("📥 Dados recebidos - Temp: {:.1}°C | Umidade: {:.1}% | Classificado: {}", 
                     sensor_data.temperature, sensor_data.humidity, classified.category_name);
            
            let mut res = response_build(&serde_json::json!({"status": "ok", "categoria": classified.category_name}).to_string());
            *res.status_mut() = StatusCode::CREATED;
            Ok(res)
        },

        // Rota GET /classificados: Lista o histórico estruturado dos dados já rotulados
        (&Method::GET, "/classificados") => {
            let classified_data = state.get_classified_data();
            let json = serde_json::to_string(&classified_data)?;
            Ok(response_build(&json))
        },

        // Rota GET /rle: Executa de forma sob-demanda a compressão RLE do lote e
        // calcula as taxas acumuladas de redução antes do envio definitivo.
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

        // Rota GET /estatisticas: Retorna a análise estatística descritiva das variáveis físicas
        (&Method::GET, "/estatisticas") => {
            let stats = state.get_statistics();
            Ok(response_build(&stats.to_string()))
        },

        // Rota POST /reset: Esvazia todos os buffers locais
        (&Method::POST, "/reset") => {
            state.reset();
            println!("🔄 Dados resetados");
            Ok(response_build(&serde_json::json!({"status": "resetado"}).to_string()))
        },

        // Opções CORS
        (&Method::OPTIONS, _) => Ok(response_build("")),

        // Qualquer outro endpoint
        _ => {
            let mut not_found = Response::default();
            *not_found.status_mut() = StatusCode::NOT_FOUND;
            Ok(not_found)
        }
    }
}

// Helper para construir respostas HTTP definindo cabeçalhos padrão do CORS
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
// FUNÇÃO PRINCIPAL (ENTRYPOINT DO FILTRO DE DADOS)
// ==============================================================================
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args: Vec<String> = env::args().collect();
    
    // Se o argumento 1 for "batch", executa a validação experimental em lote offline e finaliza.
    // O modo lote gera dados sintéticos KDE + Box-Muller e gera estatísticas de compressão em JSON.
    if args.get(1).map(|s| s.as_str()) == Some("batch") {
        let positional_args: Vec<&str> = args.iter().skip(2)
            .map(|s| s.as_str())
            .filter(|s| !s.starts_with("--"))
            .collect();
            
        let csv_path = positional_args.get(0).cloned().unwrap_or("entrada.csv");
        let output_path = positional_args.get(1).cloned().unwrap_or("estatisticas_compressao.json");
        
        let use_global_rle = args.iter().any(|s| s == "--global-rle");
        let use_synthetic = args.iter().any(|s| s == "--synthetic" || s == "--sintetico");
        
        let lote_tamanho = args.iter()
            .position(|s| s == "--tamanho-pacote" || s == "--lote-tamanho")
            .and_then(|pos| args.get(pos + 1))
            .and_then(|val| val.parse::<usize>().ok())
            .unwrap_or(6);
        
        let state = AppState::new();
        run_batch_mode(csv_path, output_path, &state, use_global_rle, use_synthetic, lote_tamanho)?;
        return Ok(());
    }

    // Configurações do servidor da névoa/relay local
    let porta_str = args.get(1)
        .cloned()
        .unwrap_or_else(|| env::var("PORTA").unwrap_or_else(|_| "8081".to_string()));
    let porta: u16 = porta_str.parse().expect("Porta inválida");

    let addr = SocketAddr::from(([0, 0, 0, 0], porta));
    
    // Cria o estado compartilhado da aplicação embrulhado em Arc
    let state = Arc::new(AppState::new());
    
    // Define a URL destino do receiver (ex: http://localhost:8082/receber)
    let receiver_url = args.get(2)
        .cloned()
        .unwrap_or_else(|| env::var("RECEIVER_URL").unwrap_or_else(|_| "http://localhost:8082/receber".to_string()));

    let ambiente = args.get(3)
        .cloned()
        .unwrap_or_else(|| env::var("AMBIENTE").unwrap_or_else(|_| "Fog".to_string()));
    
    // TAREFA EM SEGUNDO PLANO (TRANSMISSÃO PERIÓDICA):
    // Dispara uma thread assíncrona (tokio::spawn) encarregada de empacotar, comprimir via RLE
    // e enviar os dados locais coletados para o receiver a cada 30 segundos, limpando os dados locais em caso de sucesso.
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
                
                // POST para o receiver alvo
                let res: Result<reqwest::Response, reqwest::Error> = client.post(&receiver_url_clone)
                    .json(&payload)
                    .send()
                    .await;

                match res {
                    Ok(resp) => {
                        if resp.status().is_success() {
                            println!("✅ Dados enviados com sucesso. Limpando armazenamento local...");
                            state_clone.reset(); // Esvazia o buffer de sensoriamento local
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