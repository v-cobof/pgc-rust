#!/bin/bash

# Script de inicialização do ecossistema PGC-Rust
# Uso: ./start_all.sh [1|2|3]

MODE=$1

if [ -z "$MODE" ]; then
    echo "Uso: ./start_all.sh [1|2|3]"
    echo "1: Simulador(T) -> LoRa(F) -> Processador(F) -> Receiver(C)"
    echo "2: Simulador(T) -> Processador(T) -> LoRa(F) -> Receiver(C)"
    echo "3: Simulador(T) -> LoRa(F) -> Processador(C) -> Receiver(C)"
    exit 1
fi

echo "=========================================="
echo "🔨 Iniciando Build de Todos os Projetos..."
echo "=========================================="

projects="LoRa processador receiver simulador"

for project in $projects; do
    echo "🔨 Compilando $project..."
    # Usamos subshell () para o cd não afetar o diretório raiz do loop
    (cd "$project" && cargo build --target wasm32-wasip1)
    if [ $? -ne 0 ]; then
        echo "❌ Erro ao compilar $project. Abortando."
        exit 1
    fi
    echo "✅ $project OK"
done

echo "=========================================="
echo "🚀 Build concluído. Iniciando Modo: $MODE"
echo "=========================================="

case $MODE in
    1)
        echo "Configuração 1: Fluxo padrão Fog-centric"
        # Receiver no Cloud
        wt.exe -w 0 nt wsl.exe --cd "$PWD" bash -c "wasmedge --dir . receiver/target/wasm32-wasip1/debug/receiver.wasm 8082 monitoramento.txt Cloud; exec bash"
        sleep 1
        
        # Processador no Fog
        wt.exe -w 0 nt wsl.exe --cd "$PWD" bash -c "wasmedge --dir . processador/target/wasm32-wasip1/debug/processador.wasm 8081 http://localhost:8082/receber Fog; exec bash"
        sleep 1
        
        # LoRa no Fog (Relay)
        wt.exe -w 0 nt wsl.exe --cd "$PWD" bash -c "wasmedge --dir . LoRa/target/wasm32-wasip1/debug/LoRa.wasm 8080 http://localhost:8081/inserir Fog; exec bash"
        sleep 1
        
        # Simulador no Thing
        wt.exe -w 0 nt wsl.exe --cd "$PWD" bash -c "wasmedge --dir . simulador/target/wasm32-wasip1/debug/simulador.wasm http://localhost:8080/inserir Thing; exec bash"
        ;;
    
    2)
        echo "Configuração 2: Processamento no Thing"
        # Receiver no Cloud
        wt.exe -w 0 nt wsl.exe --cd "$PWD" bash -c "wasmedge --dir . receiver/target/wasm32-wasip1/debug/receiver.wasm 8082 monitoramento.txt Cloud; exec bash"
        sleep 1
        
        # LoRa no Fog (Servindo de Relay para o Cloud)
        wt.exe -w 0 nt wsl.exe --cd "$PWD" bash -c "wasmedge --dir . LoRa/target/wasm32-wasip1/debug/LoRa.wasm 8080 http://localhost:8082/receber Fog; exec bash"
        sleep 1
        
        # Processador no Thing (Envia RLE para o LoRa relay)
        wt.exe -w 0 nt wsl.exe --cd "$PWD" bash -c "wasmedge --dir . processador/target/wasm32-wasip1/debug/processador.wasm 8081 http://localhost:8080/receber Thing; exec bash"
        sleep 1
        
        # Simulador no Thing
        wt.exe -w 0 nt wsl.exe --cd "$PWD" bash -c "wasmedge --dir . simulador/target/wasm32-wasip1/debug/simulador.wasm http://localhost:8081/inserir Thing; exec bash"
        ;;
        
    3)
        echo "Configuração 3: Processamento no Cloud"
        # Receiver no Cloud
        wt.exe -w 0 nt wsl.exe --cd "$PWD" bash -c "wasmedge --dir . receiver/target/wasm32-wasip1/debug/receiver.wasm 8082 monitoramento.txt Cloud; exec bash"
        sleep 1
        
        # Processador no Cloud
        wt.exe -w 0 nt wsl.exe --cd "$PWD" bash -c "wasmedge --dir . processador/target/wasm32-wasip1/debug/processador.wasm 8081 http://localhost:8082/receber Cloud; exec bash"
        sleep 1
        
        # LoRa no Fog (Relay para o Cloud)
        wt.exe -w 0 nt wsl.exe --cd "$PWD" bash -c "wasmedge --dir . LoRa/target/wasm32-wasip1/debug/LoRa.wasm 8080 http://localhost:8081/inserir Fog; exec bash"
        sleep 1
        
        # Simulador no Thing
        wt.exe -w 0 nt wsl.exe --cd "$PWD" bash -c "wasmedge --dir . simulador/target/wasm32-wasip1/debug/simulador.wasm http://localhost:8080/inserir Thing; exec bash"
        ;;
    *)
        echo "Modo inválido: $MODE"
        exit 1
        ;;
esac
