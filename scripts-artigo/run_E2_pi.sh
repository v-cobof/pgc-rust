#!/bin/bash
# run_E2_pi.sh - Lado do Gateway (Pi) para Experimento E2 (Placement Trade-offs)
# Controla a emulação tc netem, captura de rede e execução acelerada do simulador.
# Uso: ./run_E2_pi.sh [P1|P2|stop] [SERVER_IP] [INTERFACE]

set -e

# === SETUP E COMPILAÇÃO DE DEPENDÊNCIAS ===

# 1. Instalar dependências apt (tcpdump, tshark, bc, time)
echo "📦 Verificando dependências de sistema (tcpdump, tshark, bc, time)..."
if ! command -v bc &> /dev/null || ! command -v tcpdump &> /dev/null || ! command -v tshark &> /dev/null; then
    echo "💾 Instalando pacotes necessários via apt-get..."
    sudo apt-get update
    sudo DEBIAN_FRONTEND=noninteractive apt-get install -y tcpdump tshark bc time
fi

# 2. Instalar target Rust se necessário
echo "🦀 Instalando/Verificando target Rust wasm32-wasip1..."
rustup target add wasm32-wasip1

# 3. Instalar WasmEdge se necessário
echo "🌐 Verificando WasmEdge Runtime..."
if ! command -v wasmedge &> /dev/null; then
    if [ -f "$HOME/.wasmedge/env" ]; then
        source "$HOME/.wasmedge/env"
    else
        echo "💾 Instalando WasmEdge Runtime..."
        curl -sSf https://raw.githubusercontent.com/WasmEdge/WasmEdge/master/utils/install.sh | bash
        source "$HOME/.wasmedge/env"
    fi
fi

# 4. Compilar os códigos e copiar para a raiz
echo "🛠️ Compilando processador, LoRa e simulador para WASM..."
cd processador
cargo build --target wasm32-wasip1 --release
cd ..
cd LoRa
cargo build --target wasm32-wasip1 --release
cd ..
cd simulador
cargo build --target wasm32-wasip1 --release
cd ..

cp processador/target/wasm32-wasip1/release/processador.wasm ./processador.wasm
cp LoRa/target/wasm32-wasip1/release/lora.wasm ./lora.wasm
cp simulador/target/wasm32-wasip1/release/simulador.wasm ./simulador.wasm

SCENARIO=$1
SERVER_IP=$2
INTERFACE=${3:-eth0}
INTERVAL_MS=50 # Intervalo acelerado para 50 ms por leitura (roteiro de experimentos)

PROCESSADOR_WASM="./processador.wasm"
LORA_WASM="./lora.wasm"
SIMULADOR_WASM="./simulador.wasm"

stop_all() {
    echo "🛑 Parando processos locais no Pi..."
    pkill -f "processador.wasm" || true
    pkill -f "lora.wasm" || true
    pkill -f "simulador.wasm" || true
    sudo killall tcpdump || true
    
    # Remover tc netem se existir
    echo "🧹 Removendo restrição de rede tc netem da interface $INTERFACE..."
    sudo tc qdisc del dev "$INTERFACE" root 2>/dev/null || true
    echo "✅ Limpeza concluída."
}

if [ "$SCENARIO" = "stop" ]; then
    stop_all
    exit 0
fi

# Valida parâmetros
if [ -z "$SCENARIO" ] || [ -z "$SERVER_IP" ]; then
    echo "Uso: $0 [P1|P2|stop] [SERVER_IP] (INTERFACE_REDE)"
    echo "  P1: Inicia o processador no Pi e envia dados comprimidos para a nuvem."
    echo "  P2: Inicia o gateway LoRa (relay) no Pi e envia dados brutos em JSON para a nuvem."
    echo "  INTERFACE_REDE: default 'eth0'. Interface que receberá a limitação tc netem."
    exit 1
fi

stop_all
sleep 1

# Certificar que o simulador está disponível
if [ -z "$SIMULADOR_WASM" ]; then
    echo "❌ Erro: simulador.wasm não localizado."
    exit 1
fi

OUT_DIR="resultados-artigo/E2"
mkdir -p "$OUT_DIR"
RESULT_CSV="$OUT_DIR/resultados_E2.csv"

# Cria cabeçalho do CSV se não existir
if [ ! -f "$RESULT_CSV" ]; then
    echo "scenario,run,bytes_upstream,elapsed_seconds" > "$RESULT_CSV"
fi

# Configurar emulação de rede (250 kbit, 50ms de latência)
echo "🌐 Configurando tc netem no Pi: 250kbit rate, 50ms delay na interface $INTERFACE..."
sudo tc qdisc add dev "$INTERFACE" root netem rate 250kbit delay 50ms

run_experiment_battery() {
    local label=$1
    local local_cmd=$2
    local simulator_target=$3
    
    echo "=== Iniciando Bateria de Testes para o cenário: $label ==="
    
    # Executa repetições
    for r in $(seq 1 10); do
        echo "   [Repetição $r/10]..."
        
        # 1. Inicia o componente intermediário em background (processador ou lora)
        $local_cmd > "$OUT_DIR/local_component_run_${label}_${r}.log" 2>&1 &
        local component_pid=$!
        sleep 2
        
        # 2. Inicia o tcpdump para capturar tráfego para a Nuvem
        local pcap_file="$OUT_DIR/captura_${label}_run_${r}.pcap"
        echo "      [*] Iniciando captura de rede tcpdump..."
        sudo tcpdump -i "$INTERFACE" -w "$pcap_file" "dst host $SERVER_IP" > /dev/null 2>&1 &
        local tcpdump_pid=$!
        sleep 1
        
        # 3. Executa o simulador até enviar todas as leituras de entrada.csv
        echo "      [*] Iniciando envio de dados acelerado (INTERVAL_MS = $INTERVAL_MS)..."
        t0=$(date +%s%N)
        export INTERVAL_MS=$INTERVAL_MS
        wasmedge --dir . "$SIMULADOR_WASM" "$simulator_target" > "$OUT_DIR/simulator_${label}_run_${r}.log" 2>&1 || true
        t1=$(date +%s%N)
        
        # 4. Finalizar processos locais
        sudo killall tcpdump || true
        kill $component_pid 2>/dev/null || true
        wait $component_pid 2>/dev/null || true
        
        # Calcular tempo de transmissão
        elapsed_seconds=$(echo "scale=3; ($t1 - t0) / 1000000000" | bc)
        
        # Calcular tamanho do tráfego capturado
        local bytes=0
        if command -v capinfos >/dev/null 2>&1; then
            bytes=$(capinfos -T -y "$pcap_file" 2>/dev/null | grep "File size" | awk '{print $3}')
        elif command -v tshark >/dev/null 2>&1; then
            bytes=$(tshark -r "$pcap_file" -z io,phs -q | grep "Avg Bps" | awk -F'|' '{print $2}' | tr -d ' ')
        else
            # Fallback para o tamanho do arquivo pcap
            bytes=$(stat -c %s "$pcap_file" || ls -la "$pcap_file" | awk '{print $5}')
        fi
        
        echo "      [RESULTADO] Tempo: ${elapsed_seconds}s | Tráfego: ${bytes} bytes"
        echo "$label,$r,$bytes,$elapsed_seconds" >> "$RESULT_CSV"
        sleep 2
    done
}

if [ "$SCENARIO" = "P1" ]; then
    if [ -z "$PROCESSADOR_WASM" ]; then
        echo "❌ Erro: processador.wasm não localizado."
        stop_all
        exit 1
    fi
    # P1: processador roda no Pi enviando para o server (porta 8082). Simulador envia para localhost:8081.
    export TARGET_URL="http://$SERVER_IP:8082/receber"
    run_experiment_battery "P1" "wasmedge --dir . $PROCESSADOR_WASM online" "http://127.0.0.1:8081/inserir"

elif [ "$SCENARIO" = "P2" ]; then
    if [ -z "$LORA_WASM" ]; then
        echo "❌ Erro: lora.wasm não localizado."
        stop_all
        exit 1
    fi
    # P2: lora (relay) roda no Pi encaminhando para o server (porta 8081). Simulador envia para localhost:8080.
    export PROCESSOR_URL="http://$SERVER_IP:8081/inserir"
    run_experiment_battery "P2" "wasmedge --dir . $LORA_WASM" "http://127.0.0.1:8080/inserir"
fi

stop_all
echo "✅ Experimento E2 concluído para $SCENARIO. Dados gravados em $RESULT_CSV."
