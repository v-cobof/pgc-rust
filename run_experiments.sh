#!/bin/bash
echo "=== Iniciando Experimentos com Vários Tamanhos de Pacote ==="
for size in 6 60 720 4320 20000; do
    echo "----------------------------------------"
    echo "🚀 Rodando experimento com pacote de tamanho: $size"
    wasmedge --dir . processador/target/wasm32-wasip1/debug/processador.wasm batch dados_sinteticos_gerados_rust.csv estatisticas_compressao_lote_${size}.json --tamanho-pacote ${size}
done
echo "=== Experimentos Concluídos! ==="
