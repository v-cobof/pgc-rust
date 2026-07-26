#!/bin/bash
# run_E2_server.sh - Lado do Servidor para Experimento E2 (Placement Trade-offs)
# Inicia os ouvintes da Nuvem dependendo do cenário (P1 ou P2).
# Uso: ./run_E2_server.sh [P1|P2|stop]

set -e

# Adicionar cargo ao path caso esteja no diretório padrão do rustup
if [ -d "$HOME/.cargo/bin" ]; then
    export PATH="$HOME/.cargo/bin:$PATH"
fi

# === SETUP E COMPILAÇÃO DE DEPENDÊNCIAS ===

# 1. Instalar dependências de compilação (build-essential)
echo "📦 Verificando dependências de sistema (build-essential)..."
if ! command -v gcc &> /dev/null; then
    echo "💾 Instalando build-essential via apt-get..."
    sudo apt-get update && sudo apt-get install -y build-essential
fi

# 2. Instalar target Rust se necessário
echo "🦀 Instalando/Verificando target Rust wasm32-wasip1..."
rustup target add wasm32-wasip1

# 2. Instalar WasmEdge se necessário
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

# 3. Compilar os códigos e copiar para a raiz
echo "🛠️ Compilando receiver e processador para WASM..."
cd receiver
cargo build --target wasm32-wasip1 --release
cd ..
cd processador
cargo build --target wasm32-wasip1 --release
cd ..

cp receiver/target/wasm32-wasip1/release/receiver.wasm ./receiver.wasm
cp processador/target/wasm32-wasip1/release/processador.wasm ./processador.wasm

SCENARIO=$1

RECEIVER_WASM="./receiver.wasm"
PROCESSADOR_WASM="./processador.wasm"

stop_all() {
    echo "🛑 Parando processos do experimento..."
    # Mata os processos iniciados por wasmedge
    pkill -f "receiver.wasm" || true
    pkill -f "processador.wasm" || true
    echo "✅ Ouvintes parados."
}

if [ "$SCENARIO" = "stop" ]; then
    stop_all
    exit 0
fi

if [ -z "$SCENARIO" ]; then
    echo "Uso: $0 [P1|P2|stop]"
    echo "  P1: Inicia apenas o receiver.wasm na porta 8082 (processamento na borda)"
    echo "  P2: Inicia o receiver.wasm na porta 8082 e o processador.wasm na porta 8081 (processamento na nuvem)"
    echo "  stop: Encerra todos os listeners"
    exit 1
fi

stop_all
sleep 1

# Certificar que o receiver foi compilado e está disponível
if [ -z "$RECEIVER_WASM" ]; then
    echo "❌ Erro: receiver.wasm não localizado."
    exit 1
fi

if [ "$SCENARIO" = "P1" ]; then
    echo "=== Iniciando cenário P1 (Filtro na Fog) ==="
    echo "🚀 Rodando receiver.wasm (Nuvem) na porta 8082 em background..."
    wasmedge --dir . "$RECEIVER_WASM" > receiver_server.log 2>&1 &
    
    echo "✅ Pronto! O receiver está rodando. Aguardando dados do Pi..."
    echo "Pressione Ctrl+C ou rode '$0 stop' para finalizar."
    wait

elif [ "$SCENARIO" = "P2" ]; then
    echo "=== Iniciando cenário P2 (Filtro na Cloud) ==="
    
    if [ -z "$PROCESSADOR_WASM" ]; then
        echo "❌ Erro: processador.wasm não localizado."
        exit 1
    fi
    
    echo "🚀 Rodando receiver.wasm (Nuvem) na porta 8082 em background..."
    wasmedge --dir . "$RECEIVER_WASM" > receiver_server.log 2>&1 &
    sleep 1
    
    echo "🚀 Rodando processador.wasm (Nuvem/Processador) na porta 8081 em background..."
    # Configura TARGET_URL para direcionar para o receiver local (porta 8082)
    export TARGET_URL="http://127.0.0.1:8082/receber"
    wasmedge --dir . "$PROCESSADOR_WASM" 8081 > processador_server.log 2>&1 &
    
    echo "✅ Pronto! O receiver e o processador estão rodando. Aguardando dados do Pi..."
    echo "Pressione Ctrl+C ou rode '$0 stop' para finalizar."
    wait
fi
