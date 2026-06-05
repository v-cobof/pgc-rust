import os
import pandas as pd
import numpy as np
import matplotlib.pyplot as plt
import seaborn as sns
from scipy import stats

def find_file(filename):
    if os.path.exists(filename):
        return filename
    parent_path = os.path.join("..", filename)
    if os.path.exists(parent_path):
        return parent_path
    sub_path = os.path.join("gerador-de-dados", filename)
    if os.path.exists(sub_path):
        return sub_path
    raise FileNotFoundError(f"Nao foi possivel encontrar o arquivo {filename}")

try:
    path_original = find_file("entrada.csv")
    path_rust = find_file("dados_sinteticos_gerados_rust.csv")
except FileNotFoundError as e:
    print(f"Erro: {e}")
    exit(1)

print(f"[*] Carregando dados originais de: {path_original}")
df_original = pd.read_csv(path_original)
print(f"[*] Carregando dados sinteticos do Rust de: {path_rust}")
df_rust = pd.read_csv(path_rust)

print(f"[OK] Dados originais: {len(df_original)} linhas")
print(f"[OK] Dados sinteticos Rust: {len(df_rust)} linhas")

# ============================================
# 1. TESTE DE KOLMOGOROV-SMIRNOV (KS Test)
# ============================================
print("\n[KS Test] Executando Teste KS (Kolmogorov-Smirnov)...")
print("(p-valor > 0.05 indica que as distribuicoes sao estatisticamente similares)")

resultados_ks = {}
for col in ['temperatura', 'umidade']:
    ks_stat, p_valor = stats.ks_2samp(df_original[col], df_rust[col])
    resultados_ks[col] = {'statistic': ks_stat, 'p-value': p_valor}
    
    print(f"\n{col.capitalize()}:")
    print(f"  KS statistic: {ks_stat:.4f}")
    print(f"  p-valor: {p_valor:.4f}")
    if p_valor > 0.05:
        print(f"  [OK] Distribicoes similares (p > 0.05)")
    else:
        print(f"  [ALERTA] Distribicoes podem ser diferentes (p <= 0.05)")

# ============================================
# 2. COMPARAR CORRELAÇÕES
# ============================================
print("\n[Correlation] Comparando correlacao de Pearson (Temperatura vs Umidade)...")
corr_original = df_original.corr().iloc[0,1]
corr_rust = df_rust.corr().iloc[0,1]

print(f"Correlacao nos dados originais: {corr_original:.4f}")
print(f"Correlacao nos dados sinteticos (Rust): {corr_rust:.4f}")
print(f"Diferenca absoluta: {abs(corr_original - corr_rust):.4f}")

# ============================================
# 3. GERAR GRÁFICOS DE COMPARAÇÃO
# ============================================
print("\n[Plot] Gerando graficos comparativos...")
sns.set_style("whitegrid")
fig = plt.figure(figsize=(15, 10))

# Grafico 1: Scatter plots lado a lado
ax1 = plt.subplot(2, 3, 1)
ax1.scatter(df_original['temperatura'], df_original['umidade'], alpha=0.3, s=1)
ax1.set_title('Original - Scatter Plot')
ax1.set_xlabel('Temperatura')
ax1.set_ylabel('Umidade')

ax2 = plt.subplot(2, 3, 2)
ax2.scatter(df_rust['temperatura'], df_rust['umidade'], alpha=0.3, s=1, c='green')
ax2.set_title('Rust Sintetico - Scatter Plot')
ax2.set_xlabel('Temperatura')
ax2.set_ylabel('Umidade')

# Grafico 2: Histogramas (Temperatura)
ax3 = plt.subplot(2, 3, 3)
ax3.hist(df_original['temperatura'], bins=50, alpha=0.5, label='Original', density=True)
ax3.hist(df_rust['temperatura'], bins=50, alpha=0.5, label='Rust Sintetico', density=True)
ax3.set_title('Distribuicao - Temperatura')
ax3.set_xlabel('Temperatura')
ax3.set_ylabel('Densidade')
ax3.legend()

# Grafico 3: Histogramas (Umidade)
ax4 = plt.subplot(2, 3, 4)
ax4.hist(df_original['umidade'], bins=50, alpha=0.5, label='Original', density=True)
ax4.hist(df_rust['umidade'], bins=50, alpha=0.5, label='Rust Sintetico', density=True)
ax4.set_title('Distribuicao - Umidade')
ax4.set_xlabel('Umidade')
ax4.set_ylabel('Densidade')
ax4.legend()

# Grafico 4: Box plots
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

# Grafico 5: Densidade 2D (KDE dos dados)
ax6 = plt.subplot(2, 3, 6)
sns.kdeplot(data=df_original, x='temperatura', y='umidade', ax=ax6, label='Original')
sns.kdeplot(data=df_rust, x='temperatura', y='umidade', ax=ax6, label='Rust Sintetico', linestyle='--')
ax6.set_title('Densidade 2D (KDE)')
ax6.legend()

plt.tight_layout()
output_image_name = 'comparacao_dados_rust.png'
output_image_path = os.path.join(os.path.dirname(path_rust), output_image_name)
plt.savefig(output_image_path, dpi=150, bbox_inches='tight')
print(f"[OK] Graficos salvos como: {output_image_path}")

print("\n" + "="*50)
print("RESUMO FINAL DA VALIDACAO (RUST VS ORIGINAL)")
print("="*50)
print(f"Diferenca de correlacao: {abs(corr_original - corr_rust):.4f}")
print(f"Teste KS (p-valor) Temperatura: {resultados_ks['temperatura']['p-value']:.4f}")
print(f"Teste KS (p-valor) Umidade: {resultados_ks['umidade']['p-value']:.4f}")
print("Fim da validacao!")
