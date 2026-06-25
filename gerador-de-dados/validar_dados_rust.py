import os
import pandas as pd
import numpy as np
import matplotlib.pyplot as plt
import seaborn as sns
from scipy import stats

# ==============================================================================
# FUNÇÃO AUXILIAR PARA LOCALIZAÇÃO DE ARQUIVOS
# ==============================================================================
# Tenta localizar o arquivo nos caminhos relativos mais prováveis de execução.
def find_file(filename):
    # Verifica o diretório de execução atual
    if os.path.exists(filename):
        return filename
    # Verifica um nível acima (caso executado dentro de subdiretório)
    parent_path = os.path.join("..", filename)
    if os.path.exists(parent_path):
        return parent_path
    # Verifica o caminho padrão 'gerador-de-dados'
    sub_path = os.path.join("gerador-de-dados", filename)
    if os.path.exists(sub_path):
        return sub_path
    raise FileNotFoundError(f"Não foi possível encontrar o arquivo {filename}")

# Tenta carregar o arquivo real (entrada.csv) e o sintético gerado em Rust
try:
    path_original = find_file("entrada.csv")
    path_rust = find_file("dados_sinteticos_gerados_rust.csv")
except FileNotFoundError as e:
    print(f"Erro: {e}")
    exit(1)

# ==============================================================================
# CARREGAMENTO DE DADOS
# ==============================================================================
print(f"[*] Carregando dados originais de: {path_original}")
df_original = pd.read_csv(path_original)
print(f"[*] Carregando dados sintéticos do Rust de: {path_rust}")
df_rust = pd.read_csv(path_rust)

print(f"[OK] Dados originais: {len(df_original)} linhas")
print(f"[OK] Dados sintéticos Rust: {len(df_rust)} linhas")

# ==============================================================================
# 1. TESTE DE KOLMOGOROV-SMIRNOV (KS Test)
# ==============================================================================
# Valida se os dados gerados pelo estimador estocástico em Rust/WASM coincidem
# com as distribuições reais. A hipótese nula H0 do teste KS de duas amostras
# postula que ambos os conjuntos provêm da mesma distribuição de base contínua.
# Rejeitamos a igualdade apenas se p-valor <= 0.05.
print("\n[KS Test] Executando Teste KS (Kolmogorov-Smirnov)...")
print("(p-valor > 0.05 indica que as distribuições são estatisticamente similares)")

resultados_ks = {}
for col in ['temperatura', 'umidade']:
    # Executa o teste KS bidimensional para cada coluna
    ks_stat, p_valor = stats.ks_2samp(df_original[col], df_rust[col])
    resultados_ks[col] = {'statistic': ks_stat, 'p-value': p_valor}
    
    print(f"\n{col.capitalize()}:")
    print(f"  KS statistic: {ks_stat:.4f}")
    print(f"  p-valor: {p_valor:.4f}")
    if p_valor > 0.05:
        print(f"  [OK] Distribuições similares (p > 0.05)")
    else:
        print(f"  [ALERTA] Distribuições podem ser diferentes (p <= 0.05)")

# ==============================================================================
# 2. COMPARAR CORRELAÇÕES
# ==============================================================================
# Calcula a correlação linear de Pearson para verificar se o gerador
# estocástico multivariado implementado em Rust manteve a interdependência física
# inversa esperada entre a temperatura e a umidade.
print("\n[Correlation] Comparando correlação de Pearson (Temperatura vs Umidade)...")
corr_original = df_original.corr().iloc[0,1]
corr_rust = df_rust.corr().iloc[0,1]

print(f"Correlação nos dados originais: {corr_original:.4f}")
print(f"Correlação nos dados sintéticos (Rust): {corr_rust:.4f}")
print(f"Diferença absoluta: {abs(corr_original - corr_rust):.4f}")

# ==============================================================================
# 3. GERAR GRÁFICOS DE COMPARAÇÃO
# ==============================================================================
# Renderiza e exporta gráficos comparativos das duas bases para análise visual.
print("\n[Plot] Gerando gráficos comparativos...")
sns.set_style("whitegrid")
fig = plt.figure(figsize=(15, 10))

# Subgráfico 1: Dispersão (Scatter Plot) de dados originais (temperatura vs umidade)
ax1 = plt.subplot(2, 3, 1)
ax1.scatter(df_original['temperatura'], df_original['umidade'], alpha=0.3, s=1)
ax1.set_title('Original - Scatter Plot')
ax1.set_xlabel('Temperatura')
ax1.set_ylabel('Umidade')

# Subgráfico 2: Dispersão (Scatter Plot) de dados sintéticos do Rust
ax2 = plt.subplot(2, 3, 2)
ax2.scatter(df_rust['temperatura'], df_rust['umidade'], alpha=0.3, s=1, c='green')
ax2.set_title('Rust Sintético - Scatter Plot')
ax2.set_xlabel('Temperatura')
ax2.set_ylabel('Umidade')

# Subgráfico 3: Histograma comparativo de densidade para Temperatura
ax3 = plt.subplot(2, 3, 3)
ax3.hist(df_original['temperatura'], bins=50, alpha=0.5, label='Original', density=True)
ax3.hist(df_rust['temperatura'], bins=50, alpha=0.5, label='Rust Sintético', density=True)
ax3.set_title('Distribuição - Temperatura')
ax3.set_xlabel('Temperatura')
ax3.set_ylabel('Densidade')
ax3.legend()

# Subgráfico 4: Histograma comparativo de densidade para Umidade
ax4 = plt.subplot(2, 3, 4)
ax4.hist(df_original['umidade'], bins=50, alpha=0.5, label='Original', density=True)
ax4.hist(df_rust['umidade'], bins=50, alpha=0.5, label='Rust Sintético', density=True)
ax4.set_title('Distribuição - Umidade')
ax4.set_xlabel('Umidade')
ax4.set_ylabel('Densidade')
ax4.legend()

# Subgráfico 5: Boxplot comparando a distribuição estatística de ambas as bases
ax5 = plt.subplot(2, 3, 5)
dados_box = pd.DataFrame({
    'Original_Temp': df_original['temperatura'],
    'Rust_Temp': df_rust['temperatura'],
    'Original_Umid': df_original['umidade'],
    'Rust_Umid': df_rust['umidade']
})
dados_box.boxplot(ax=ax5)
ax5.set_title('Box Plot Comparison')
ax5.tick_params(axis='x', rotation=45)

# Subgráfico 6: Curva de densidade de contorno (2D) estimada (KDE)
ax6 = plt.subplot(2, 3, 6)
sns.kdeplot(data=df_original, x='temperatura', y='umidade', ax=ax6, label='Original')
sns.kdeplot(data=df_rust, x='temperatura', y='umidade', ax=ax6, label='Rust Sintético', linestyle='--')
ax6.set_title('Densidade 2D (KDE)')
ax6.legend()

# Salva a imagem final da comparação estatística no mesmo diretório do arquivo gerado
plt.tight_layout()
output_image_name = 'comparacao_dados_rust.png'
output_image_path = os.path.join(os.path.dirname(path_rust), output_image_name)
plt.savefig(output_image_path, dpi=150, bbox_inches='tight')
plt.close()
print(f"[OK] Gráficos salvos como: {output_image_path}")

# ==============================================================================
# RESUMO FINAL
# ==============================================================================
print("\n" + "="*50)
print("RESUMO FINAL DA VALIDAÇÃO (RUST VS ORIGINAL)")
print("="*50)
print(f"Diferença de correlação: {abs(corr_original - corr_rust):.4f}")
print(f"Teste KS (p-valor) Temperatura: {resultados_ks['temperatura']['p-value']:.4f}")
print(f"Teste KS (p-valor) Umidade: {resultados_ks['umidade']['p-value']:.4f}")
print("Fim da validação!")
