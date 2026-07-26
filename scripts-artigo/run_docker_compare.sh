#!/bin/bash
# run_docker_compare.sh - Comparação de Footprint: WasmEdge Bare-Metal vs Docker (Rust Nativo) (Seção V-C)
# Cria a imagem Docker com o binário Rust Nativo, mede o tempo de cold start do container e o consumo de memória RAM.

set -e

# Adicionar cargo ao path caso esteja no diretório padrão do rustup
if [ -d "$HOME/.cargo/bin" ]; then
    export PATH="$HOME/.cargo/bin:$PATH"
fi

# Configurações
REPETICOES=10
OUT_DIR="resultados-artigo/E3"
mkdir -p "$OUT_DIR"
DOCKER_CSV="$OUT_DIR/docker_resultados.csv"

echo "run,engine,startup_ms,memory_rss_mb" > "$DOCKER_CSV"

# Verificar se o Docker está instalado
if ! command -v docker &> /dev/null; then
    echo "❌ Erro: Docker não está instalado ou o usuário atual não tem permissão para rodá-lo (adicione ao grupo docker)."
    exit 1
fi

# 1. Instalar dependências de compilação (build-essential)
echo "📦 Verificando dependências de sistema (build-essential)..."
if ! command -v gcc &> /dev/null; then
    echo "💾 Instalando build-essential via apt-get..."
    sudo apt-get update && sudo apt-get install -y build-essential
fi

# 2. Certificar que o processador_nativo está compilado nativamente para Linux
NATIVO_BIN="./processador_nativo"
if [ ! -f "$NATIVO_BIN" ]; then
    echo "🛠️ Compilando processador nativo local (Linux ELF)..."
    cd processador
    cargo build --release
    cd ..
    cp processador/target/release/processador ./processador_nativo
fi

# 3. Gerar o Dockerfile dinamicamente na raiz
echo "🐳 Gerando Dockerfile temporário com Rust Nativo..."
cat << 'EOF' > Dockerfile.tmp
FROM debian:slim

# Instalar curl para o health check/polling do script
RUN apt-get update && apt-get install -y curl && rm -rf /var/lib/apt/lists/*

# Copiar o binário nativo compilado para o Linux e o CSV de entrada
COPY processador_nativo /app/processador_nativo
COPY entrada.csv /app/entrada.csv

WORKDIR /app
EXPOSE 8081

# Comando de boot do servidor nativo na porta 8081
CMD ["./processador_nativo", "8081"]
EOF

# 4. Compilar a imagem Docker
echo "🐳 Compilando a imagem Docker (docker-native-compare)..."
docker build -f Dockerfile.tmp -t docker-native-compare:latest .
rm -f Dockerfile.tmp

# Função para medir o cold start do Docker
measure_docker() {
    echo "🚀 Iniciando medições com Docker (Rust Nativo)..."
    
    # 1. Rodada de aquecimento (Warm-up) - Descartar
    echo "   [Aquecimento] Inicializando container Docker..."
    docker run -d -p 8081:8081 --name test-docker-warmup docker-native-compare:latest > /dev/null
    while ! curl -s -o /dev/null http://localhost:8081/estatisticas; do
        sleep 0.01
    done
    docker rm -f test-docker-warmup > /dev/null
    sleep 2

    # 2. Bateria de repetições
    for i in $(seq 1 $REPETICOES); do
        echo "   [Repetição $i/$REPETICOES]..."
        
        t0=$(date +%s%N)
        # Sobe o container em background
        docker run -d -p 8081:8081 --name "test-docker-$i" docker-native-compare:latest > /dev/null
        
        # Polling até responder HTTP 200 OK na rota de estatísticas
        while ! curl -s -o /dev/null http://localhost:8081/estatisticas; do
            sleep 0.001
        done
        t1=$(date +%s%N)
        
        # Coleta o consumo de memória RAM do container usando docker stats (em MB)
        mem_raw=$(docker stats "test-docker-$i" --no-stream --format "{{.MemUsage}}" | awk '{print $1}')
        # Remover sufixos de unidade (MiB, MB, GiB) para ficar apenas o valor numérico decimal
        mem_val=$(echo "$mem_raw" | sed -e 's/MiB//g' -e 's/MB//g' -e 's/GiB/ * 1024/g' -e 's/KiB/\/1024/g')
        mem_mb=$(echo "scale=2; $mem_val" | bc)

        # Remove o container
        docker rm -f "test-docker-$i" > /dev/null
        
        # Calcular startup_ms
        diff_ms=$(( (t1 - t0) / 1000000 ))
        
        echo "      [RESULTADO] Startup: ${diff_ms}ms | RAM: ${mem_mb} MB"
        echo "$i,docker_native,$diff_ms,$mem_mb" >> "$DOCKER_CSV"
        sleep 1
    done
}

measure_docker

# Limpar imagem construída
echo "🧹 Limpando imagem docker-native-compare..."
docker rmi -f docker-native-compare:latest > /dev/null || true

echo "✅ Experimento Docker concluído. Resultados salvos em $DOCKER_CSV"
