#!/bin/bash
# run_E1.sh - Portabilidade e Performance (Seção V-A)
# Executa 10 repetições de processamento em lote para: Nativo, WASM Interpretado e WASM AOT.
# Salva dados brutos em resultados-artigo/E1/

set -e

# Adicionar cargo ao path caso esteja no diretório padrão do rustup
if [ -d "$HOME/.cargo/bin" ]; then
    export PATH="$HOME/.cargo/bin:$PATH"
fi

# === SETUP E COMPILAÇÃO DE DEPENDÊNCIAS ===

# 1. Instalar dependências apt (time, bc)
echo "📦 Verificando dependências de sistema (time, bc)..."
if ! command -v bc &> /dev/null || [ ! -f /usr/bin/time ]; then
    echo "💾 Instalando pacotes necessários via apt-get..."
    sudo apt-get update && sudo apt-get install -y time bc
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

# Configurações
REPETICOES=10
DADO_CSV="entrada.csv"
TAMANHO_LOTE=720
OUT_DIR="resultados-artigo/E1"
mkdir -p "$OUT_DIR"

RESULTADOS_CSV="$OUT_DIR/resultados_E1.csv"
echo "run,engine,elapsed_seconds,max_rss_kb" > "$RESULTADOS_CSV"

# Verificar se o arquivo CSV de entrada existe
if [ ! -f "$DADO_CSV" ]; then
    echo "Erro: Arquivo $DADO_CSV não encontrado na raiz."
    exit 1
fi

# Determinar qual arquitetura está rodando
ARCH=$(uname -m)
echo "=== Iniciando Experimento E1 no host ($ARCH) ==="

# Localizar binários no workspace
NATIVO_BIN="./processador_nativo"
WASM_BIN="./processador.wasm"

# Função para rodar e capturar métricas de tempo e RSS
run_benchmark() {
    local engine=$1
    local cmd=$2
    
    echo "🚀 Rodando $engine..."
    
    # 1. Aquecimento (Warm-up) - Descartar
    echo "   [Aquecimento] Executando primeira vez..."
    $cmd > /dev/null 2>&1
    
    # 2. Bateria de repetições
    for i in $(seq 1 $REPETICOES); do
        echo "   [Repetição $i/$REPETICOES]..."
        
        # Executa usando /usr/bin/time -v e salva a saída em temp_time.log
        /usr/bin/time -v -o temp_time.log $cmd > /dev/null 2>&1
        
        # Parse das métricas de tempo e memória
        rss=$(grep "Maximum resident set size" temp_time.log | awk '{print $NF}')
        elapsed_raw=$(grep "Elapsed (wall clock) time" temp_time.log | awk '{print $NF}')
        
        # Converter tempo para segundos decimais (formato H:MM:SS ou MM:SS)
        parts=$(echo "$elapsed_raw" | tr ':' ' ')
        num_parts=$(echo "$parts" | wc -w)
        if [ "$num_parts" -eq 3 ]; then
            h=$(echo "$parts" | awk '{print $1}')
            m=$(echo "$parts" | awk '{print $2}')
            s=$(echo "$parts" | awk '{print $3}')
            elapsed=$(echo "$h * 3600 + $m * 60 + $s" | bc)
        else
            m=$(echo "$parts" | awk '{print $1}')
            s=$(echo "$parts" | awk '{print $2}')
            elapsed=$(echo "$m * 60 + $s" | bc)
        fi
        
        # Salva no CSV
        echo "$i,$engine,$elapsed,$rss" >> "$RESULTADOS_CSV"
    done
    rm -f temp_time.log
}

# Execução das baterias
if [ -f "$NATIVO_BIN" ]; then
    run_benchmark "nativo" "$NATIVO_BIN batch $DADO_CSV stats.json --tamanho-pacote $TAMANHO_LOTE"
else
    echo "⚠️  Binário nativo não localizado em $NATIVO_BIN. Pulando benchmark nativo..."
fi

if [ -f "$WASM_BIN" ]; then
    run_benchmark "wasm_interpretado" "wasmedge --dir . $WASM_BIN batch $DADO_CSV stats.json --tamanho-pacote $TAMANHO_LOTE"
    
    echo "⚙️ Compilando $WASM_BIN para AOT..."
    wasmedge compile "$WASM_BIN" processador_aot.wasm
    run_benchmark "wasm_aot" "wasmedge --dir . processador_aot.wasm batch $DADO_CSV stats.json --tamanho-pacote $TAMANHO_LOTE"
    rm -f processador_aot.wasm
else
    echo "❌ Arquivo processador.wasm não localizado em $WASM_BIN. Certifique-se de compilá-lo com target wasm32-wasip1."
fi

echo "✅ Experimento E1 concluído. Resultados salvos em $RESULTADOS_CSV"
