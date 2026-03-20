use rand::Rng;
use tokio::time::{self, Duration, Instant};
use serde::{Deserialize, Serialize};
use chrono::Timelike;

#[derive(Debug, Serialize, Deserialize)]
struct SensorData {
    temperature: f32,
    humidity: f32,
    timestamp: u64,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), reqwest::Error> {
    let interval = Duration::from_secs(5); // Envia a cada 5 segundos
    let mut next_time = Instant::now() + interval;
    
    let client = reqwest::Client::new();
    let mut rng = rand::thread_rng();

    println!("Iniciando simulador de sensores - Gerando dados de temperatura e umidade...");
    println!("Enviando para http://127.0.0.1:8081/inserir a cada 5 segundos\n");

    loop {
        // Gera dados baseados nos ranges do artigo
        // Round 1 (madrugada): temperatura entre 14-18°C, umidade entre 53-75%
        // Round 2 (manhã): temperatura entre 15-28°C, umidade entre 23-68%
        
        // Simulando variação ao longo do dia (24 horas)
        let hora_atual = chrono::Local::now().hour();
        
        let (temp_min, temp_max, hum_min, hum_max) = match hora_atual {
            0..=6 => (14.0, 18.0, 53.0, 75.0),      // Madrugada - Round 1
            7..=12 => (15.0, 28.0, 23.0, 68.0),     // Manhã - Round 2
            13..=18 => (22.0, 32.0, 30.0, 60.0),    // Tarde - mais quente
            _ => (16.0, 22.0, 45.0, 70.0),          // Noite - ameno
        };

        // Gera valores aleatórios dentro dos ranges
        let temperature: f32 = rng.gen_range(temp_min..=temp_max);
        let humidity: f32 = rng.gen_range(hum_min..=hum_max);
        
        // Arredonda para 2 casas decimais (como no dataset real)
        let temperature = (temperature * 100.0).round() / 100.0;
        let humidity = (humidity * 100.0).round() / 100.0;
        
        // Timestamp atual em segundos
        let timestamp = chrono::Utc::now().timestamp() as u64;

        let sensor_data = SensorData {
            temperature,
            humidity,
            timestamp,
        };

        println!("📊 Dados gerados - Hora: {}h | Temp: {:.1}°C | Umidade: {:.1}%", 
                 hora_atual, temperature, humidity);

        // Envia os dados como JSON
        let res = client
            .post("http://127.0.0.1:8081/inserir")
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