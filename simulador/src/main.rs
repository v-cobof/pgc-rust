use rand::Rng;
use tokio::time::{self, Duration, Instant};
use serde::{Deserialize, Serialize};
use chrono::Timelike;
use csv::ReaderBuilder;

#[derive(Debug, Serialize, Deserialize)]
struct SensorData {
    temperature: f32,
    humidity: f32,
    timestamp: u64,
}

#[derive(Debug, Deserialize)]
struct CsvRecord {
    temperatura: f32,
    umidade: f32,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), reqwest::Error> {
    let args: Vec<String> = std::env::args().collect();
    let target_url = args.get(1)
        .cloned()
        .unwrap_or_else(|| std::env::var("TARGET_URL").unwrap_or_else(|_| "http://127.0.0.1:8080/inserir".to_string()));

    // Ambiente: argumento 2 ou env AMBIENTE ou Thing
    let ambiente = args.get(2)
        .cloned()
        .unwrap_or_else(|| std::env::var("AMBIENTE").unwrap_or_else(|_| "Thing".to_string()));

    // Carregar dados de entrada.csv
    let csv_path = "entrada.csv";
    println!("📖 Carregando dados de {}...", csv_path);
    
    let mut reader = ReaderBuilder::new()
        .has_headers(true)
        .from_path(csv_path)
        .expect("Arquivo entrada.csv não encontrado ou inacessível. Certifique-se de usar --dir . no WasmEdge");
    
    let records: Vec<CsvRecord> = reader.deserialize()
        .collect::<Result<Vec<CsvRecord>, csv::Error>>()
        .expect("Erro ao ler/parsear entrada.csv");
    
    let mut csv_index = 0;
    println!("✅ {} registros carregados do CSV.", records.len());

    let interval = Duration::from_secs(5); // Envia a cada 5 segundos
    let mut next_time = Instant::now() + interval;
    
    let client = reqwest::Client::new();
    let mut _rng = rand::thread_rng(); // Prefixo _ para evitar warning de não uso

    println!("Iniciando simulador de sensores - Usando dados do arquivo CSV...");
    println!("🌍 Ambiente: {}", ambiente);
    println!("Enviando para {} a cada 5 segundos\n", target_url);

    loop {
        let hora_atual = chrono::Local::now().hour();

        /* --- BLOCADO: Lógica de Geração Aleatória Original ---
        let (temp_min, temp_max, hum_min, hum_max) = match hora_atual {
            0..=6 => (14.0, 18.0, 53.0, 75.0),
            7..=12 => (15.0, 28.0, 23.0, 68.0),
            13..=18 => (22.0, 32.0, 30.0, 60.0),
            _ => (16.0, 22.0, 45.0, 70.0),
        };
        let temperature: f32 = _rng.gen_range(temp_min..=temp_max);
        let humidity: f32 = _rng.gen_range(hum_min..=hum_max);
        let temperature = (temperature * 100.0).round() / 100.0;
        let humidity = (humidity * 100.0).round() / 100.0;
        ---------------------------------------------------- */

        // Pegar próximo registro do CSV (Circular)
        let record = &records[csv_index];
        let temperature = record.temperatura;
        let humidity = record.umidade;
        csv_index = (csv_index + 1) % records.len();
        
        let timestamp = chrono::Utc::now().timestamp() as u64;

        let sensor_data = SensorData {
            temperature,
            humidity,
            timestamp,
        };

        println!("📊 [Linha CSV {}] Hora: {}h | Temp: {:.1}°C | Umidade: {:.1}%", 
                 csv_index, hora_atual, temperature, humidity);

        // Envia os dados como JSON
        let res = client
            .post(&target_url)
            .json(&sensor_data)
            .send()
            .await?;

        if res.status().is_success() {
            println!("✅ Dados enviados com sucesso");
        } else {
            println!("❌ Erro no envio: {}", res.status());
        }

        // Aguarda até o próximo envio
        time::sleep(next_time - Instant::now()).await;
        next_time += interval;
    }
}
