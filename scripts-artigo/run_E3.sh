#!/bin/bash
# run_E3.sh - Footprint do Runtime (Seção V-C)
# Mede tamanhos dos binários e tempo de cold start (inicialização do servidor).
# Salva dados brutos em resultados-artigo/E3/

set -e

# Adicionar cargo ao path caso esteja no diretório padrão do rustup
if [ -d "$HOME/.cargo/bin" ]; then
    export PATH="$HOME/.cargo/bin:$PATH"
fi

# === SETUP E COMPILAÇÃO DE DEPENDÊNCIAS ===

# 1. Instalar dependências apt (curl, build-essential)
echo "📦 Verificando dependências de sistema (curl, build-essential)..."
if ! command -v curl &> /dev/null || ! command -v gcc &> /dev/null; then
    echo "💾 Instalando pacotes necessários via apt-get..."
    sudo apt-get update && sudo apt-get install -y curl build-essential
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
echo "🛠️ Compilando processador Rust para WASM..."
cd processador
cargo build --target wasm32-wasip1 --release
cd ..
cp processador/target/wasm32-wasip1/release/processador.wasm ./processador.wasm

echo "🛠️ Tentando compilar nativo local (opcional)..."
if cd processador && cargo build --release >/dev/null 2>&1; then
    cd ..
    cp processador/target/release/processador ./processador_nativo
else
    cd ..
    echo "⚠️  Compilação nativa falhou ou não é suportada diretamente no host. Pulando nativo..."
fi

REPETICOES=10
OUT_DIR="resultados-artigo/E3"
mkdir -p "$OUT_DIR"

TAMANHOS_CSV="$OUT_DIR/tamanhos.csv"
COLD_START_CSV="$OUT_DIR/cold_start.csv"

echo "engine,size_bytes" > "$TAMANHOS_CSV"
echo "run,engine,startup_ms" > "$COLD_START_CSV"

# 1. Localização dos arquivos
NATIVO_BIN="./processador_nativo"
WASM_BIN="./processador.wasm"

# Compilar AOT se WASM estiver disponível
WASM_AOT=""
# Compilar AOT se WASM estiver disponível
WASM_AOT=""
if [ -f "$WASM_BIN" ]; then
    echo "⚙️ Compilando para AOT..."
    wasmedge compile "$WASM_BIN" processador_aot.wasm
    WASM_AOT="processador_aot.wasm"
fi

# Registrar tamanhos dos arquivos
if [ -f "$NATIVO_BIN" ]; then
    echo "nativo,$(stat -c %s "$NATIVO_BIN")" >> "$TAMANHOS_CSV"
fi
if [ -f "$WASM_BIN" ]; then
    echo "wasm_interpretado,$(stat -c %s "$WASM_BIN")" >> "$TAMANHOS_CSV"
fi
if [ -f "$WASM_AOT" ]; then
    echo "wasm_aot,$(stat -c %s "$WASM_AOT")" >> "$TAMANHOS_CSV"
fi

# Função para medir tempo de cold start (porta 8081)
measure_cold_start() {
    local engine=$1
    local run_cmd=$2
    
    echo "🚀 Medindo cold start para $engine..."
    
    # Aquecimento (warm-up) - Descartar
    echo "   [Aquecimento] Inicializando servidor..."
    $run_cmd > /dev/null 2>&1 &
    local warm_pid=$!
    # Polling até responder 200 OK ou responder conexão recusada
    while ! curl -s -o /dev/null http://localhost:8081/estatisticas; do
        sleep 0.01
    done
    kill $warm_pid
    wait $warm_pid 2>/dev/null || true
    sleep 1
    
    # Rodadas de testes
    for i in $(seq 1 $REPETICOES); do
        echo "   [Repetição $i/$REPETICOES]..."
        
        t0=$(date +%s%N)
        $run_cmd > /dev/null 2>&1 &
        local pid=$!
        
        # Polling até que a rota de estatísticas responda HTTP 200 OK
        while ! curl -s -o /dev/null http://localhost:8081/estatisticas; do
            sleep 0.001
        done
        t1=$(date +%s%N)
        
        # Finaliza o processo
        kill $pid
        wait $pid 2>/dev/null || true
        
        diff_ms=$(( (t1 - t0) / 1000000 ))
        echo "$i,$engine,$diff_ms" >> "$COLD_START_CSV"
        sleep 1
    done
}

# Medir cold start para cada motor
if [ -f "$NATIVO_BIN" ]; then
    measure_cold_start "nativo" "$NATIVO_BIN 8081"
fi
if [ -f "$WASM_BIN" ]; then
    measure_cold_start "wasm_interpretado" "wasmedge --dir . $WASM_BIN 8081"
fi
if [ -f "$WASM_AOT" ]; then
    measure_cold_start "wasm_aot" "wasmedge --dir . $WASM_AOT 8081"
    rm -f processador_aot.wasm
fi

echo "✅ Experimento E3 concluído. Resultados salvos em $OUT_DIR/"
